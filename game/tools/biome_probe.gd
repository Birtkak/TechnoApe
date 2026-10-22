extends SceneTree

## Headless probe for the biome rules against the native generator's climate.
## Feeds WorldGen.column_info noises/heights into BiomeRules (with latitude and
## lapse applied) and reports:
##  - a north-south transect: biome mix per 4 km band from far south to far north
##  - agreement with the native biome_at() for the 13 original biomes near spawn
##  - observed border pairs over a region, and any pair the rules say cannot touch
##  - a biome map PNG of the rules' classification (colors from biomes.json)
##   godot_console.exe --headless --path game --script res://tools/biome_probe.gd -- seed=1337 radius=2048

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
	var reg := BiomeRegistry.load_default()
	if not reg.errors.is_empty():
		for e in reg.errors:
			printerr(e)
		quit(1)
		return
	var rules := BiomeRules.new(reg)
	var gen := WorldGen.new()
	gen.configure(seed_value, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)

	# Transect: x in -2 km..2 km, z from +48 km (south) to -48 km (north), 4 km bands.
	var transect := []
	for band in range(12, -12, -1):
		var hist := {}
		var n := 0
		var z0 := band * 8000
		for i in 400:
			var x := -4000 + (i % 20) * 400
			var z := z0 - (i / 20) * 400
			var name := _classify(rules, gen, x, z)
			hist[name] = hist.get(name, 0) + 1
			n += 1
		transect.append({"z_km_from": z0 / 2000, "z_km_to": (z0 - 8000) / 2000, "mix_pct": _pct(hist, n)})

	# Agreement with the native classification near spawn (no latitude there, so the
	# only expected differences are the new biomes and the re-tuned band edges).
	var agree := 0
	var total := 0
	var diffs := {}
	var step := maxi(1, radius / 128)
	var grid := {}
	for z in range(-radius, radius, step):
		for x in range(-radius, radius, step):
			var name := _classify(rules, gen, x, z)
			grid[Vector2i(x, z)] = name
			var native: String = WorldGen.biome_name(gen.biome_at(x, z))
			total += 1
			if native == name:
				agree += 1
			else:
				var k := "%s->%s" % [native, name]
				diffs[k] = diffs.get(k, 0) + 1
	# Border pairs actually observed on the grid vs. what the rules allow.
	var pairs := {}
	var illegal := {}
	for z in range(-radius, radius - step, step):
		for x in range(-radius, radius - step, step):
			var a: String = grid[Vector2i(x, z)]
			for nb in [Vector2i(x + step, z), Vector2i(x, z + step)]:
				var b: String = grid[nb]
				if a == b:
					continue
				var key := "%s|%s" % [a, b] if a < b else "%s|%s" % [b, a]
				pairs[key] = pairs.get(key, 0) + 1
				if not rules.can_border(a, b):
					illegal[key] = pairs[key]

	# Underground: cave biome mix per depth band over the same grid (cave noise: slow 3D noise).
	var cave_noise := FastNoiseLite.new()
	cave_noise.seed = seed_value + 7
	cave_noise.noise_type = FastNoiseLite.TYPE_SIMPLEX_SMOOTH
	cave_noise.frequency = 1.0 / (float(reg.underground["cave_noise_period_m"]) / Settings.VOXEL_M)
	var cave_mix := {}
	for dn in reg.depth_band_names():
		cave_mix[dn] = {}
	for z in range(-radius, radius, step * 2):
		for x in range(-radius, radius, step * 2):
			var info: Dictionary = gen.column_info(x, z)
			if int(info["height"]) < Settings.SEA_LEVEL_VOXEL - 1:
				continue
			var above: int = int(info["height"]) - Settings.SEA_LEVEL_VOXEL
			var col := {"temperature": rules.temperature(float(info["temperature"]), float(z), above), "humidity": float(info["humidity"]), "above_sea": above}
			for di in reg.depth_band_names().size():
				var band: Dictionary = reg.underground["depth_bands"][di]
				var depth := int(band["min"]) + 10
				var y := int(info["height"]) - depth
				var name := rules.classify_3d_name(col, depth, cave_noise.get_noise_3d(x, y, z))
				var dn: String = band["name"]
				cave_mix[dn][name] = cave_mix[dn].get(name, 0) + 1
	for dn in cave_mix:
		var tot := 0
		for k in cave_mix[dn]:
			tot += cave_mix[dn][k]
		cave_mix[dn] = _pct(cave_mix[dn], maxi(1, tot))

	# Map image of the rules' classification.
	var mstep := maxi(1, radius / 256)
	var n := 2 * radius / mstep
	var img := Image.create(n, n, false, Image.FORMAT_RGB8)
	for py in n:
		for px in n:
			var name := _classify(rules, gen, -radius + px * mstep, -radius + py * mstep)
			var c: Array = reg.get_def(name)["color"]
			img.set_pixel(px, py, Color8(int(c[0]), int(c[1]), int(c[2])))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://out"))
	img.save_png("res://out/biomes_%d.png" % seed_value)

	var top_pairs := pairs.keys()
	top_pairs.sort_custom(func(a, b): return pairs[a] > pairs[b])
	var top := {}
	for k in top_pairs.slice(0, 15):
		top[k] = pairs[k]
	print(JSON.stringify({
		"test": "biome_probe", "seed": seed_value,
		"transect_south_to_north": transect,
		"native_agreement_pct": snappedf(100.0 * agree / total, 0.1),
		"native_vs_rules_diffs": diffs,
		"border_pairs_observed": pairs.size(), "top_border_pairs": top,
		"illegal_border_pairs": illegal,
		"cave_mix_pct_by_depth": cave_mix,
		"map": "res://out/biomes_%d.png (%d px = %d m across)" % [seed_value, n, int(2 * radius * Settings.VOXEL_M)],
	}))
	quit(0)


func _classify(rules: BiomeRules, gen: WorldGen, x: int, z: int) -> String:
	var info: Dictionary = gen.column_info(x, z)
	var above: int = int(info["height"]) - Settings.SEA_LEVEL_VOXEL
	# column_info "temperature" is the final climate (latitude and lapse applied by the
	# native generator since GEN_VERSION 6); "temperature_noise" is the raw noise + jitter.
	var t := rules.temperature(float(info.get("temperature_noise", info["temperature"])), float(z), above)
	return rules.classify_name({
		"temperature": t, "humidity": float(info["humidity"]), "above_sea": above,
		"continentalness": float(info["continentalness"]), "erosion": float(info["erosion"]),
		"slope": float(info.get("slope", 0.0)), "river": float(info.get("river", 0.0)),
		"lake": bool(info.get("lake", false)) and int(info["height"]) < int(info.get("water_level", -1)),
	})


func _pct(hist: Dictionary, total: int) -> Dictionary:
	var out := {}
	for k in hist:
		out[k] = snappedf(100.0 * hist[k] / total, 0.1)
	return out
