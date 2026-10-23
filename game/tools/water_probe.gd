extends SceneTree

## Boat check for rivers and the sea (2026-10-23). Prints one JSON line:
##   rivers: width (m) and depth (m) across sampled river centre lines, by size
##   ocean: depth histogram (m below sea level) over an 8 km square, deep share
##   shore: depth every 16 m along a line from the nearest beach out to sea
## Run: godot --headless --path game --script res://tools/water_probe.gd [-- seed=N]

const SEA := 256


func _init() -> void:
	var seed_v := 1337
	for a in OS.get_cmdline_user_args():
		if a.begins_with("seed="):
			seed_v = int(a.substr(5))
	var g := WorldGen.new()
	g.configure(seed_v, SEA, 768)
	var out := {"seed": seed_v, "rivers": _rivers(g), "ocean": _ocean(g), "shore": _shore(g)}
	print(JSON.stringify(out))
	quit(0)


## Walks a grid; at each river centre column (river mask ~1) measures the wet
## width across x and z and the water depth over the bed.
func _rivers(g: WorldGen) -> Dictionary:
	var widths := []
	var depths := []
	var narrow := 0
	var shallow := 0
	for gz in range(-4000, 4001, 64):
		for gx in range(-4000, 4001, 64):
			var c: Dictionary = g.column_info(gx, gz)
			if float(c["river"]) < 0.98 or int(c["water_level"]) < 0 or bool(c["lake"]) or int(c["river_bed"]) < 0:
				continue
			var depth := int(c["water_level"]) - int(c["river_bed"]) + 1
			var w := mini(_wet_run(g, gx, gz, Vector2i(1, 0)), _wet_run(g, gx, gz, Vector2i(0, 1)))
			widths.append(w * 0.5)
			depths.append(depth * 0.5)
			if w * 0.5 < 3.0:
				narrow += 1
			if depth * 0.5 < 1.0:
				shallow += 1
	widths.sort()
	depths.sort()
	var n := widths.size()
	if n == 0:
		return {"samples": 0}
	return {"samples": n, "width_m": {"min": widths[0], "p10": widths[n / 10], "p50": widths[n / 2], "max": widths[n - 1]},
		"depth_m": {"min": depths[0], "p10": depths[n / 10], "p50": depths[n / 2], "max": depths[n - 1]},
		"under_3m_wide": narrow, "under_1m_deep": shallow}


func _wet_run(g: WorldGen, x: int, z: int, d: Vector2i) -> int:
	var n := 1
	for s in [1, -1]:
		for i in range(1, 80):
			var c: Dictionary = g.column_info(x + d.x * i * s, z + d.y * i * s)
			if int(c["water_level"]) < 0 or int(c["river_bed"]) >= int(c["water_level"]) + 1:
				break
			n += 1
	return n


func _ocean(g: WorldGen) -> Dictionary:
	var hist := {}
	var deep := 0
	var sea := 0
	var deepest := 0
	for gz in range(-8000, 8001, 128):
		for gx in range(-8000, 8001, 128):
			var c: Dictionary = g.column_info(gx, gz)
			var h := int(c["height"])
			if h >= SEA - 1:
				continue
			sea += 1
			var m := (SEA - h) * 0.5
			deepest = maxi(deepest, SEA - h)
			var band := "%d-%d m" % [int(m / 10) * 10, int(m / 10) * 10 + 10]
			hist[band] = int(hist.get(band, 0)) + 1
			if m >= 40.0:
				deep += 1
	return {"columns": sea, "deep_40m_share": snappedf(float(deep) / maxi(sea, 1), 0.01), "deepest_m": deepest * 0.5, "hist": hist}


## From the nearest beach, step toward lower ground until the sea is 1 km wide.
func _shore(g: WorldGen) -> Array:
	var b: Vector3i = g.find_nearest("beach", 0, 0, 4096)
	if b.y < 0:
		return []
	var best := Vector2.ZERO
	var lowest := 1 << 30
	for a in 16:
		var dir := Vector2(cos(a * TAU / 16), sin(a * TAU / 16))
		var h := int(g.column_info(b.x + int(dir.x * 200), b.z + int(dir.y * 200))["height"])
		if h < lowest:
			lowest = h
			best = dir
	var out := []
	for i in range(0, 64):
		var x := b.x + int(best.x * i * 32)
		var z := b.z + int(best.y * i * 32)
		var c: Dictionary = g.column_info(x, z)
		out.append([i * 16, (int(c["height"]) - SEA) * 0.5, c["biome_name"]])
	return out
