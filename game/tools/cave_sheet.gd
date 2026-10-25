extends SceneTree

## Debug: compare cave variants without a window (WorldGen.set_variant slot 1). One row per variant: a
## top-down map of how much of each column is cave (brighter = more, red = a cave mouth on the surface)
## and a vertical slice west-east through the centre, as generated (sky, cave air black, rock grey).
##   godot --headless --path game --script res://tools/cave_sheet.gd -- seed=1337 [x=0 z=0] [km=1]
##       [variants=0,1] [slice_m=480] [depth_m=140] [out=res://out/cave_sheet.png]
## Prints the cave share and mouths per km^2 of each variant as JSON.

const Settings := preload("res://scripts/settings.gd")

const N := 320
const SLICE_W := 960
const COLORS := {0: Color(0.03, 0.03, 0.04), 1: Color(0.5, 0.5, 0.52), 2: Color(0.45, 0.33, 0.22), 3: Color(0.33, 0.55, 0.25),
	5: Color(0.2, 0.4, 0.75), 23: Color(0.2, 0.4, 0.75), 7: Color(1.0, 0.45, 0.1), 13: Color(0.15, 0.15, 0.15),
	53: Color(0.6, 0.9, 0.1), 54: Color(0.6, 0.9, 0.1), 55: Color(0.9, 0.85, 0.2), 15: Color(0.55, 0.5, 0.48), 22: Color(0.55, 0.5, 0.48)}
const SKY := Color(0.75, 0.83, 0.92)


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(int(args.get("seed", "1337")), Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var km := float(args.get("km", "1"))
	var cx := int(args.get("x", "0"))
	var cz := int(args.get("z", "0"))
	var variants: Array = Array(String(args.get("variants", "0,1")).split(",")).map(func(v): return int(v))
	var slice_v := int(Settings.m2v(float(args.get("slice_m", "480"))))
	var depth_v := int(Settings.m2v(float(args.get("depth_m", "140"))))
	var step := int(Settings.m2v(km * 1000.0) / N)
	var ds := maxi(1, slice_v / SLICE_W)
	var slice_h := depth_v / ds
	var row_h := maxi(N, slice_h)
	var sheet := Image.create(N + SLICE_W, row_h * variants.size(), false, Image.FORMAT_RGB8)
	sheet.fill(Color(0.1, 0.1, 0.1))
	var report := []
	for vi in variants.size():
		gen.get_macro().set_variant(1, variants[vi])
		var t0 := Time.get_ticks_msec()
		var y0 := vi * row_h
		# top-down cave share and mouths
		var cm := gen.cave_map(cx - N / 2 * step, cz - N / 2 * step, N, step)
		var share := 0.0
		var mouths := 0
		for j in N:
			for i in N:
				var k := (j * N + i) * 2
				var v := cm[k] / 255.0
				share += v
				var open := cm[k + 1] == 1
				if open:
					mouths += 1
				sheet.set_pixel(i, y0 + j, Color(1, 0.15, 0.1) if open else Color(v * 2.5, v * 2.5, v * 2.0).clamp())
		# vertical slice from the highest ground on the line down depth_m
		var x0 := cx - slice_v / 2
		var top := 0
		for i in range(0, slice_v, 16):
			top = maxi(top, gen.height_at(x0 + i, cz))
		top += 8
		var bytes: PackedByteArray = gen.generate_block(Vector3i(x0, top - depth_v, cz), Vector3i(slice_v, depth_v, 1), 0, 2)
		var cave_px := 0
		for i in SLICE_W:
			var x := i * ds
			if x >= slice_v:
				break
			var ground := gen.height_at(x0 + x, cz)
			for py in slice_h:
				var y := depth_v - 1 - py * ds
				var m := 0 if bytes.is_empty() else bytes.decode_u16((y + depth_v * x) * 2)
				var wy := top - depth_v + y
				var c: Color
				if m == 0 and wy >= ground:
					c = SKY
				else:
					c = COLORS.get(m, Color(0.4, 0.42, 0.35))
					if m == 0:
						cave_px += 1
				sheet.set_pixel(N + i, y0 + py, c)
		var area_km2 := pow(N * step * Settings.VOXEL_M / 1000.0, 2.0)
		# mouths: 8-connected blobs of open columns
		var seen := PackedByteArray()
		seen.resize(N * N)
		var blobs := 0
		for k0 in N * N:
			if cm[k0 * 2 + 1] != 1 or seen[k0] == 1:
				continue
			blobs += 1
			var stack := [k0]
			seen[k0] = 1
			while not stack.is_empty():
				var k: int = stack.pop_back()
				for dj in range(-1, 2):
					for di in range(-1, 2):
						var ii: int = k % N + di
						var jj: int = k / N + dj
						if ii < 0 or jj < 0 or ii >= N or jj >= N:
							continue
						var kk := jj * N + ii
						if cm[kk * 2 + 1] == 1 and seen[kk] == 0:
							seen[kk] = 1
							stack.append(kk)
		report.append({"variant": variants[vi], "cave_share": snappedf(share / (N * N), 0.001),
			"mouth_columns": mouths, "mouths_per_km2": snappedf(blobs / area_km2, 0.1), "slice_cave_px": cave_px, "ms": Time.get_ticks_msec() - t0})
	var out := String(args.get("out", "res://out/cave_sheet.png"))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(out.get_base_dir()))
	sheet.save_png(out)
	print(JSON.stringify({"out": out, "rows": report}))
	quit(0)
