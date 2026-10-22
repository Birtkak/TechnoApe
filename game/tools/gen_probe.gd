extends SceneTree

## Headless probe for the native generator: timing per block at LOD 0/2,
## sanity statistics over a region, and a biome map PNG for Claude to look at.
##   godot_console.exe --headless --path game --script res://tools/gen_probe.gd -- seed=1337 radius=2048

var seed_value := 1337
var radius := 2048


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"seed": seed_value = int(kv[1])
			"radius": radius = int(kv[1])
	var gen := WorldGen.new()
	gen.configure(seed_value, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)

	# Timing: 512 LOD0 blocks around the surface, 512 LOD2 blocks.
	var t0 := Time.get_ticks_usec()
	var nonempty := 0
	for i in 512:
		var x := (i % 8) * 16
		var z := ((i / 8) % 8) * 16
		var y := (gen.height_at(x, z) / 16) * 16 - 16 + (i / 64 - 4) * 16
		if not gen.generate_block(Vector3i(x, y, z), Vector3i(16, 16, 16), 0).is_empty():
			nonempty += 1
	var lod0_us := (Time.get_ticks_usec() - t0) / 512.0
	t0 = Time.get_ticks_usec()
	for i in 512:
		var x := (i % 8) * 64
		var z := ((i / 8) % 8) * 64
		var y := (gen.height_at(x, z) / 64) * 64 - 64 + (i / 64 - 4) * 64
		gen.generate_block(Vector3i(x, y, z), Vector3i(16, 16, 16), 2)
	var lod2_us := (Time.get_ticks_usec() - t0) / 512.0

	# Sanity over the map region: height range, biome histogram, water share.
	var hist := {}
	var hmin := 99999
	var hmax := -1
	var samples := 0
	var step := maxi(1, radius / 128)
	for z in range(-radius, radius, step):
		for x in range(-radius, radius, step):
			var info: Dictionary = gen.column_info(x, z)
			hist[info["biome_name"]] = hist.get(info["biome_name"], 0) + 1
			hmin = mini(hmin, info["height"])
			hmax = maxi(hmax, info["height"])
			samples += 1
	# Cave share in one 128^3 region below the surface at the origin.
	var air_under := 0
	var total_under := 0
	var h0 := gen.height_at(0, 0)
	for cz in 8:
		for cy in 8:
			for cx in 8:
				var origin := Vector3i(cx * 16 - 64, h0 - 160 + cy * 16, cz * 16 - 64)
				var b: PackedByteArray = gen.generate_block(origin, Vector3i(16, 16, 16), 0)
				if b.is_empty():
					air_under += 4096
				else:
					for i in 4096:
						if b[i * 2] == 0 and b[i * 2 + 1] == 0:
							air_under += 1
				total_under += 4096

	# Map image
	var mstep := maxi(1, radius / 256)
	var px: PackedByteArray = gen.map_image(0, 0, radius, mstep)
	var n := 2 * radius / mstep
	var img := Image.create_from_data(n, n, false, Image.FORMAT_RGB8, px)
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://out"))
	img.save_png("res://out/map_%d.png" % seed_value)

	print(JSON.stringify({
		"test": "gen_probe", "seed": seed_value,
		"lod0_us_per_block": snappedf(lod0_us, 0.1), "lod2_us_per_block": snappedf(lod2_us, 0.1),
		"lod0_nonempty_of_512": nonempty,
		"height_min": hmin, "height_max": hmax, "sea_level": Settings.SEA_LEVEL_VOXEL,
		"biomes_pct": _pct(hist, samples),
		"cave_air_pct_under_origin": snappedf(100.0 * air_under / total_under, 0.1),
		"spawn_height": h0, "spawn_biome": WorldGen.biome_name(gen.biome_at(0, 0)),
		"map": "res://out/map_%d.png (%d px = %d m across)" % [seed_value, n, int(2 * radius * Settings.VOXEL_M)],
	}))
	quit(0)


func _pct(hist: Dictionary, total: int) -> Dictionary:
	var out := {}
	for k in hist:
		out[k] = snappedf(100.0 * hist[k] / total, 0.1)
	return out
