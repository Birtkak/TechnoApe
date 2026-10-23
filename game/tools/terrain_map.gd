extends SceneTree

## Debug: a hillshaded height map of the world around a point, plus the terrain mix
## (water, plains, hills, mountains by local relief), to judge the world's shape without flying it.
##   godot --headless --path game --script res://tools/terrain_map.gd -- [km=8] [px=512] [x=0 z=0] [out=res://out/terrain_map.png]

const Settings := preload("res://scripts/settings.gd")


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(int(args.get("seed", "1337")), Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var km := float(args.get("km", "8"))
	var px := int(args.get("px", "512"))
	var cx := int(args.get("x", "0"))
	var cz := int(args.get("z", "0"))
	var step := int(Settings.m2v(km * 1000.0) / px)
	var sea: int = Settings.SEA_LEVEL_VOXEL
	var t0 := Time.get_ticks_msec()
	var h := PackedFloat32Array()
	h.resize(px * px)
	for j in px:
		for i in px:
			h[j * px + i] = gen.height_at(cx + (i - px / 2) * step, cz + (j - px / 2) * step) * Settings.VOXEL_M
	var img := Image.create(px, px, false, Image.FORMAT_RGB8)
	var counts := {"water": 0, "plains": 0, "hills": 0, "rugged": 0, "mountains": 0}
	var hmax := -INF
	var sea_m := sea * Settings.VOXEL_M
	var cell := step * Settings.VOXEL_M
	for j in px:
		for i in px:
			var v := h[j * px + i]
			hmax = maxf(hmax, v)
			var l := h[j * px + maxi(i - 1, 0)]
			var u := h[maxi(j - 1, 0) * px + i]
			var shade := clampf(0.6 + ((v - l) + (v - u)) / cell * 0.6, 0.15, 1.0)
			# local relief: the spread of heights within about 64 m
			var r := 2 if cell < 32.0 else 1
			var lo := v
			var hi := v
			for dj in range(-r, r + 1):
				for di in range(-r, r + 1):
					var w := h[clampi(j + dj, 0, px - 1) * px + clampi(i + di, 0, px - 1)]
					lo = minf(lo, w)
					hi = maxf(hi, w)
			var col: Color
			if v <= sea_m + 0.01:
				counts["water"] += 1
				col = Color(0.2, 0.35, 0.6)
			else:
				var rel := hi - lo
				var kind := "plains" if rel < 3.0 else ("hills" if rel < 12.0 else ("rugged" if rel < 40.0 else "mountains"))
				counts[kind] += 1
				var a := clampf((v - sea_m) / 220.0, 0.0, 1.0)
				col = Color(0.35, 0.6, 0.3).lerp(Color(0.55, 0.5, 0.4), clampf(a * 2.0, 0.0, 1.0)).lerp(Color(0.95, 0.95, 0.95), clampf(a * 2.0 - 1.0, 0.0, 1.0))
				col = col * shade
			img.set_pixel(i, j, col)
	var out: String = args.get("out", "res://out/terrain_map.png")
	img.save_png(ProjectSettings.globalize_path(out))
	var total := float(px * px)
	var mix := {}
	for k in counts:
		mix[k] = snappedf(counts[k] / total, 0.001)
	print(JSON.stringify({"km": km, "max_above_sea_m": snappedf(hmax - sea_m, 0.1), "mix": mix, "ms": Time.get_ticks_msec() - t0, "out": out}))
	quit(0)
