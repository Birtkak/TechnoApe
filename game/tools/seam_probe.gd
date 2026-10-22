extends SceneTree

## Headless probe: are rivers continuous across hydrology tile borders?
## Walks along the x = 4096 seam (tile 0 | tile 1) and the z = 4096 seam,
## compares the river at one voxel on each side, and reports the largest and
## mean jumps in bed height, water level and river mask. Also samples the
## same columns at 1 and 12 cells inside each tile to show the blend band.
##   godot_console.exe --headless --path game --script res://tools/seam_probe.gd -- seed=1337

var seed_value := 1337


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "seed":
			seed_value = int(kv[1]) if kv[1].is_valid_int() else _seed_from_string(kv[1])
	var gen := WorldGen.new()
	gen.configure(seed_value, 256, 768)
	var report := {"test": "seam_probe", "seed": seed_value}
	for axis in ["x", "z"]:
		var crossings := 0
		var max_bed := 0
		var max_water := 0
		var sum_bed := 0.0
		var max_mask := 0.0
		var lost := 0  # river on one side, none on the other
		for k in range(-4096, 4096, 2):
			var a: Dictionary
			var b: Dictionary
			if axis == "x":
				a = gen.column_info(4095, k)
				b = gen.column_info(4096, k)
			else:
				a = gen.column_info(k, 4095)
				b = gen.column_info(k, 4096)
			var ra: float = a["river"]
			var rb: float = b["river"]
			if ra < 0.3 and rb < 0.3:
				continue
			crossings += 1
			if (ra >= 0.3) != (rb >= 0.3):
				lost += 1
			var db: int = absi(int(a["height"]) - int(b["height"]))
			var wa: int = a["water_level"]
			var wb: int = b["water_level"]
			var dw: int = absi(wa - wb) if (wa >= 0 and wb >= 0) else 0
			max_bed = maxi(max_bed, db)
			max_water = maxi(max_water, dw)
			sum_bed += db
			max_mask = maxf(max_mask, absf(ra - rb))
		report["seam_" + axis] = {
			"river_columns": crossings, "lost_one_side": lost,
			"max_height_jump": max_bed, "mean_height_jump": snappedf(sum_bed / maxi(1, crossings), 0.01),
			"max_water_jump": max_water, "max_mask_jump": snappedf(max_mask, 0.01),
		}
	print(JSON.stringify(report))
	quit(0)


func _seed_from_string(s: String) -> int:
	var h := 2166136261
	for b in s.to_utf8_buffer():
		h = ((h ^ b) * 16777619) & 0xFFFFFFFF
	return h
