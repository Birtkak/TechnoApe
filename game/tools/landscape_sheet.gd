extends SceneTree

## Debug: compare generator variants without a window. For each variant (WorldGen.set_variant) one row:
## a hillshaded top-down map, a perspective view toward the highest point (a software height-field
## render, Comanche style, so it runs headless) and the height profile through that point.
##   godot --headless --path game --script res://tools/landscape_sheet.gd -- seed=1337 [x=0 z=0] [km=6]
##       [rows=0:384:0,1:384:0,2:384:0] [look=centre] [dist_m=1600 up_m=60] [out=res://out/landscape_sheet.png]
## x, z in voxels; rows = variant:world height m:relief m (0 = the variant's own), variant 0 = the ranges of
## ADR 0035, 1 massif, 2 horns (WorldGen.set_variant slots 0 and 2).

const Settings := preload("res://scripts/settings.gd")

const N := 320            # map samples per side
const VIEW_W := 960
const VIEW_H := 400
const PROFILE_H := 160
const COVER := [Color(0.36, 0.55, 0.27), Color(0.47, 0.45, 0.43), Color(0.93, 0.94, 0.96), Color(0.82, 0.76, 0.52), Color(0.2, 0.4, 0.17)]
const WATER := Color(0.2, 0.36, 0.55)
const SKY := Color(0.75, 0.83, 0.92)


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	var km := float(args.get("km", "6"))
	var cx := int(args.get("x", "0"))
	var cz := int(args.get("z", "0"))
	var variants: Array = Array(String(args.get("rows", "0:384:0,1:384:0,2:384:0")).split(","))
	var seed := int(args.get("seed", "1337"))
	var step := int(Settings.m2v(km * 1000.0) / N)
	var x0 := cx - N / 2 * step
	var z0 := cz - N / 2 * step
	var row_h := maxi(N, VIEW_H + PROFILE_H)
	var sheet := Image.create(N + VIEW_W, row_h * variants.size(), false, Image.FORMAT_RGB8)
	sheet.fill(Color(0.1, 0.1, 0.1))
	var report := []
	for vi in variants.size():
		var spec: PackedStringArray = String(variants[vi]).split(":")
		gen.configure(seed, Settings.SEA_LEVEL_VOXEL, int(Settings.m2v(float(spec[1]))))
		gen.get_macro().set_variant(0, int(spec[0]))
		gen.get_macro().set_variant(2, int(spec[2]))
		var t0 := Time.get_ticks_msec()
		var sm := gen.surface_map(x0, z0, N, step)
		var h := PackedFloat32Array()
		var top := PackedFloat32Array()   # water surface where there is water, else the ground (metres)
		var cover := PackedInt32Array()
		h.resize(N * N)
		top.resize(N * N)
		cover.resize(N * N)
		var best := 0
		for k in N * N:
			h[k] = sm[k * 3] * Settings.VOXEL_M
			top[k] = maxf(h[k], sm[k * 3 + 1] * Settings.VOXEL_M) if sm[k * 3 + 1] >= 0 else h[k]
			cover[k] = sm[k * 3 + 2] if sm[k * 3 + 1] < 0 else -1
			if h[k] > h[best]:
				best = k
		var cell := step * Settings.VOXEL_M
		var y0 := vi * row_h
		_map(sheet, 0, y0, top, cover, cell)
		var bi := best % N
		var bj := best / N
		if args.get("look", "") == "centre":   # look at the centre instead of the highest point
			bi = N / 2
			bj = N / 2
		_view(sheet, N, y0, top, cover, cell, bi, bj, float(args.get("dist_m", "1600")), float(args.get("up_m", "60")))
		_profile(sheet, N, y0 + VIEW_H, top, cover, bj, cell)
		var sea_m := Settings.SEA_LEVEL_VOXEL * Settings.VOXEL_M
		report.append({"variant": variants[vi], "peak_m_above_sea": snappedf(h[best] - sea_m, 0.1),
			"peak_x": x0 + bi * step, "peak_z": z0 + bj * step, "ms": Time.get_ticks_msec() - t0})
	var out := String(args.get("out", "res://out/landscape_sheet.png"))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(out.get_base_dir()))
	sheet.save_png(out)
	print(JSON.stringify({"out": out, "rows": report}))
	quit(0)


func _shade(top: PackedFloat32Array, i: int, j: int, cell: float) -> float:
	var l := top[j * N + maxi(i - 1, 0)]
	var u := top[maxi(j - 1, 0) * N + i]
	var v := top[j * N + i]
	return clampf(0.75 + ((v - l) + (v - u)) / cell * 0.5, 0.25, 1.15)


func _color(cover: PackedInt32Array, k: int) -> Color:
	return WATER if cover[k] < 0 else COVER[cover[k]]


func _map(img: Image, ox: int, oy: int, top: PackedFloat32Array, cover: PackedInt32Array, cell: float) -> void:
	for j in N:
		for i in N:
			img.set_pixel(ox + i, oy + j, _color(cover, j * N + i) * _shade(top, i, j, cell))


## The view stands on low dry land dist_m (1.6 km) from the highest point and looks at it, up_m (60 m) up.
func _view(img: Image, ox: int, oy: int, top: PackedFloat32Array, cover: PackedInt32Array, cell: float, pi: int, pj: int, dist_m: float, up_m: float) -> void:
	var target := Vector2(pi, pj)
	var dist := dist_m / cell
	var sea_m := Settings.SEA_LEVEL_VOXEL * Settings.VOXEL_M
	var cam := Vector2(-1, -1)
	var best := INF
	for k in 32:
		var p := target + Vector2.from_angle(TAU * k / 32.0) * dist
		if p.x < 1 or p.y < 1 or p.x > N - 2 or p.y > N - 2:
			continue
		var hh := _sample(top, p.x, p.y)
		if hh > sea_m + 1.0 and hh < best:
			best = hh
			cam = p
	if cam.x < 0:
		cam = (target - Vector2(dist, 0)).clamp(Vector2(1, 1), Vector2(N - 2, N - 2))
	var dir := (target - cam).normalized()
	var cam_h := _sample(top, cam.x, cam.y) + up_m
	var fov := 1.0
	var horizon := VIEW_H * 0.42
	var scale := VIEW_H * 0.9
	for x in VIEW_W:
		var a := (float(x) / VIEW_W - 0.5) * fov
		var rd := dir.rotated(a)
		var ybuf := VIEW_H
		var d := 1.0
		while d < N * 1.2 and ybuf > 0:
			var p := cam + rd * d
			if p.x < 0 or p.y < 0 or p.x >= N - 1 or p.y >= N - 1:
				break
			var hh := _sample(top, p.x, p.y)
			var sy := int((cam_h - hh) / (d * cell * cos(a)) * scale + horizon)
			if sy < ybuf:
				var k := int(p.y) * N + int(p.x)
				var fog := clampf(d / (N * 1.1), 0.0, 0.85)
				var c := (_color(cover, k) * _shade(top, int(p.x), int(p.y), cell)).lerp(SKY, fog)
				for y in range(maxi(sy, 0), ybuf):
					img.set_pixel(ox + x, oy + y, c)
				ybuf = sy
			d += 0.25 + d * 0.004
		for y in range(0, maxi(ybuf, 0)):
			img.set_pixel(ox + x, oy + y, SKY)


## Height profile west-east through the highest point from sea level, heights x2 against distance.
func _profile(img: Image, ox: int, oy: int, top: PackedFloat32Array, cover: PackedInt32Array, pj: int, cell: float) -> void:
	var lo := Settings.SEA_LEVEL_VOXEL * Settings.VOXEL_M
	var m_per_px := cell * N / VIEW_W
	for x in VIEW_W:
		var i := int(float(x) / VIEW_W * N)
		var v := top[pj * N + i]
		var y := int(PROFILE_H - 4 - (v - lo) / m_per_px * 2.0)
		for yy in range(PROFILE_H):
			img.set_pixel(ox + x, oy + yy, _color(cover, pj * N + i) * 0.8 if yy >= y else Color(0.12, 0.12, 0.14))


func _sample(top: PackedFloat32Array, x: float, y: float) -> float:
	var i := int(x)
	var j := int(y)
	var tx := x - i
	var ty := y - j
	var a := top[j * N + i]
	var b := top[j * N + i + 1]
	var c := top[(j + 1) * N + i]
	var d := top[(j + 1) * N + i + 1]
	return lerpf(lerpf(a, b, tx), lerpf(c, d, tx), ty)
