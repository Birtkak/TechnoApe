extends SceneTree

## Debug: how the land climbs (bug reports 2026-10-25: lego steps, easy mountains, shallow sea, trees over
## ravines). One JSON line:
##   steps: rises between neighbouring land columns (fine voxels) around a point, as a histogram, the share
##     of rises that are a single voxel and the share at or above a jump (Settings.jump_height_m)
##   mountain: the same around a mountain point, plus the share of 1 m moves that need more than a jump
##   ocean: sea depth (m) over a square of macro columns: percentiles, deep shares, max
##   cave: floor rises and pools in a box round the cave report spot (x, cy, z)
##   trees: trunks near the nearest ravines, how many stand on carved ground (air under the trunk foot), and
##     each ravine's cross-section (depth, top width, rise over run of each wall's upper half)
##   bench=1 instead: ms per surface block at 400 land columns (generation cost where the ground really is)
##   godot --headless --path game --script res://tools/relief_probe.gd -- seed=1579829108
##       [x=15710 z=19678] [mx=-15686 mz=-8491] [ox=-4126 oz=-1876] [r_m=96] [ocean_km=8] [ravines=4]

const Settings := preload("res://scripts/settings.gd")
const JUMP := 10    # voxels: Settings.jump_height_m 1.25 m
const STEP := 4     # voxels: Settings.step_height_m 0.55 m, the auto step-up


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	var seed_v := int(args.get("seed", "1579829108"))
	gen.configure(seed_v, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var r := int(Settings.m2v(float(args.get("r_m", "96"))))
	var out := {"seed": seed_v}
	if args.get("bench", "0") == "1":
		# bench=1: ms per surface block (the 16^3 block holding the ground) at 400 land columns over 4 km,
		# so terrain that moved up or down is still timed where it is (fine_gen_bench times fixed heights)
		print(JSON.stringify(_bench(gen)))
		quit(0)
		return
	var t0 := Time.get_ticks_msec()
	out["steps"] = _steps(gen, int(args.get("x", "15710")), int(args.get("z", "19678")), r)
	out["mountain"] = _steps(gen, int(args.get("mx", "-15686")), int(args.get("mz", "-8491")), r * 2)
	out["cave"] = _cave(gen, int(args.get("x", "15710")), int(args.get("cy", "1181")), int(args.get("z", "19678")))
	out["ocean"] = _ocean(gen, int(args.get("ox", "-4126")), int(args.get("oz", "-1876")), float(args.get("ocean_km", "8")))
	out["trees"] = _trees(gen, int(args.get("ravines", "4")), "ravine")
	out["trees_control"] = _trees(gen, 2, "old_growth_forest")
	out["ms"] = Time.get_ticks_msec() - t0
	print(JSON.stringify(out))
	quit(0)


func _bench(gen: FineGen) -> Dictionary:
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	var pts := []
	var span := int(Settings.m2v(4000.0))
	while pts.size() < 400:
		var x := rng.randi_range(-span / 2, span / 2)
		var z := rng.randi_range(-span / 2, span / 2)
		var h := gen.height_at(x, z)
		if h > Settings.SEA_LEVEL_VOXEL + 2:
			pts.append(Vector3i(x, h, z))
	var res := {}
	for lod in [0, 1, 2, 4]:
		var s: int = 1 << lod
		var times := []
		for p in pts:
			var o := Vector3i((p.x >> (4 + lod)) << (4 + lod), ((p.y - 8 * s) >> (4 + lod)) << (4 + lod), (p.z >> (4 + lod)) << (4 + lod))
			var t := Time.get_ticks_usec()
			gen.generate_block(o, Vector3i(16, 16, 16), lod, 2)
			times.append(float(Time.get_ticks_usec() - t) / 1000.0)
		times.sort()
		var sum := 0.0
		for v in times:
			sum += v
		res["lod%d" % lod] = {"avg_ms": snappedf(sum / times.size(), 0.001), "p50": snappedf(times[times.size() / 2], 0.001), "p90": snappedf(times[times.size() * 9 / 10], 0.001)}
	return res


## Cave floors in a 128 x 64 x 128 box round (cx, cy, cz): the highest floor voxel under the box top per
## column (solid under air, below the ground), rises between neighbouring floors, and the cave pools on them
## (liquid floor voxels: count and the biggest pool as connected columns).
func _cave(gen: FineGen, cx: int, cy: int, cz: int) -> Dictionary:
	var liquid := {}
	for m in range(1, 512):
		var nm := GameMaterials.name_of(m)
		if nm.contains("water") or nm.contains("sludge") or nm.contains("lava"):
			liquid[m] = true
	const N := 128
	const H := 64
	var x0 := cx - N / 2
	var z0 := cz - N / 2
	var y0 := ((cy - H / 2) >> 4) << 4
	var vox := PackedInt32Array()
	vox.resize(N * N * H)
	for bz in range(0, N, 16):
		for bx in range(0, N, 16):
			for by in range(0, H, 16):
				var bytes: PackedByteArray = gen.generate_block(Vector3i(x0 + bx, y0 + by, z0 + bz), Vector3i(16, 16, 16), 0, 2)
				if bytes.is_empty():
					continue
				for i in 4096:
					var x := bx + (i / 16) % 16
					var y := by + i % 16
					var z := bz + i / 256
					vox[(z * N + x) * H + y] = bytes.decode_u16(i * 2)
	var floor_y := PackedInt32Array()
	floor_y.resize(N * N)
	var pool := {}
	for z in N:
		for x in N:
			floor_y[z * N + x] = -1
			var ground := gen.height_at(x0 + x, z0 + z)
			for y in range(H - 2, 0, -1):
				if y0 + y >= ground - 4:
					continue
				var m := vox[(z * N + x) * H + y]
				if m != 0 and vox[(z * N + x) * H + y + 1] == 0:
					floor_y[z * N + x] = y
					if liquid.has(m):
						pool[Vector2i(x, z)] = true
					break
	var hist := {"0": 0, "1": 0, "2-3": 0, "4-7": 0, "8+": 0}
	var rises := 0
	var ones := 0
	for z in N:
		for x in N - 1:
			for d in [Vector2i(1, 0), Vector2i(0, 1)]:
				if z + d.y >= N:
					continue
				var a := floor_y[z * N + x]
				var b := floor_y[(z + d.y) * N + x + d.x]
				if a < 0 or b < 0 or absi(a - b) > 16:
					continue
				var s := absi(a - b)
				if s == 0:
					hist["0"] += 1
				else:
					rises += 1
					if s == 1:
						hist["1"] += 1
						ones += 1
					elif s < 4:
						hist["2-3"] += 1
					elif s < 8:
						hist["4-7"] += 1
					else:
						hist["8+"] += 1
	var biggest := 0
	var seen := {}
	for p in pool:
		if seen.has(p):
			continue
		var stack := [p]
		seen[p] = true
		var n := 0
		while not stack.is_empty():
			var q: Vector2i = stack.pop_back()
			n += 1
			for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
				var r: Vector2i = q + d
				if pool.has(r) and not seen.has(r):
					seen[r] = true
					stack.append(r)
		biggest = maxi(biggest, n)
	return {"at": [cx, cy, cz], "floor_hist": hist, "single_voxel_share": snappedf(float(ones) / maxf(1.0, float(rises)), 0.001),
		"pool_columns": pool.size(), "biggest_pool_columns": biggest, "biggest_pool_m2": snappedf(biggest * Settings.VOXEL_M * Settings.VOXEL_M, 0.01)}


## A ravine's cross-section along x through p: the carved ground per column (top solid voxel), its top width,
## depth, and how steep each wall's upper half is (rise over run from the rim to half the depth).
func _ravine_profile(gen: FineGen, p: Vector3i, along_z := false) -> Dictionary:
	const W := 256
	var x0 := p.x - W / 2
	var top := PackedInt32Array()
	var ground := PackedInt32Array()
	top.resize(W)
	ground.resize(W)
	var lo := 1 << 30
	var hi := -1
	for i in W:
		ground[i] = gen.height_at(p.x, p.z - W / 2 + i) if along_z else gen.height_at(x0 + i, p.z)
		lo = mini(lo, ground[i])
		hi = maxi(hi, ground[i])
		top[i] = -1
	var ylo := ((lo - int(Settings.m2v(70.0))) >> 4) << 4
	for bx in range(0, W, 16):
		for by in range(ylo, hi + 16, 16):
			var o := Vector3i(p.x, by, p.z - W / 2 + bx) if along_z else Vector3i(x0 + bx, by, p.z)
			var bytes: PackedByteArray = gen.generate_block(o, Vector3i(16, 16, 16), 0, 2)
			if bytes.is_empty():
				continue
			for i in 4096:
				if (along_z and (i / 16) % 16 != 0) or (not along_z and i / 256 != 0):
					continue
				var m := bytes.decode_u16(i * 2)
				var x := bx + (i / 256 if along_z else (i / 16) % 16)
				var y := by + i % 16
				if m != 0 and y <= ground[x] and y > top[x]:
					top[x] = y
	var c := W / 2
	var bottom := c
	for i in W:
		if ground[i] - top[i] > ground[bottom] - top[bottom]:
			bottom = i
	var depth := ground[bottom] - top[bottom]
	if OS.get_environment("RELIEF_DBG") != "":
		var s := []
		for i in range(0, W, 4):
			s.append(ground[i] - top[i])
		print("profile ", p, " z=", along_z, " ", s)
	if depth < 16:
		return {"depth_m": snappedf(Settings.v2m(float(depth)), 0.1)}
	# depth per column with gaps of a few columns closed (grass, sticks and pools inside read as no cut)
	var d := PackedInt32Array()
	d.resize(W)
	for i in W:
		var m := 0
		for k in range(maxi(0, i - 3), mini(W, i + 4)):
			m = maxi(m, ground[k] - top[k])
		d[i] = m
	var walls := []
	var rims := []
	for dir in [-1, 1]:
		var i := bottom
		while i > 0 and i < W - 1 and d[i] > 4:
			i += dir
		var rim := i
		var j := rim
		while d[j] < depth / 2 and j != bottom:
			j -= dir
		rims.append(rim)
		# a cut with no wall on this side (the deepest column is a cave under the rim): not measured
		walls.append(snappedf(float(depth / 2) / maxf(1.0, float(absi(rim - j))), 0.01) if absi(rim - bottom) > 2 else null)
	return {"depth_m": snappedf(Settings.v2m(float(depth)), 0.1), "top_width_m": snappedf(Settings.v2m(float(absi(rims[1] - rims[0]))), 0.1),
		"upper_wall_slope": walls}


func _steps(gen: FineGen, cx: int, cz: int, r: int) -> Dictionary:
	var sea := Settings.SEA_LEVEL_VOXEL
	var jump := JUMP
	var hist := {"0": 0, "1": 0, "2-3": 0, "4-7": 0, "8-15": 0, "16+": 0}
	var rises := 0
	var ones := 0
	var over_jump := 0
	var over_step := 0
	var n := 0
	var row := PackedInt32Array()
	row.resize(2 * r + 1)
	var prev := PackedInt32Array()
	# every 2nd row and column pair: rises along x and z between neighbouring columns
	for j in range(-r, r + 1, 2):
		for i in range(-r, r + 1):
			row[i + r] = gen.height_at(cx + i, cz + j)
		var below := PackedInt32Array()
		below.resize(2 * r + 1)
		for i in range(-r, r + 1, 2):
			below[i + r] = gen.height_at(cx + i, cz + j + 1)
		for i in range(-r, r, 2):
			for d in [absi(row[i + r + 1] - row[i + r]), absi(below[i + r] - row[i + r])]:
				if row[i + r] <= sea + 2:
					continue
				n += 1
				if d == 0:
					hist["0"] += 1
				else:
					rises += 1
					if d == 1:
						hist["1"] += 1
						ones += 1
					elif d < 4:
						hist["2-3"] += 1
					elif d < 8:
						hist["4-7"] += 1
					elif d < 16:
						hist["8-15"] += 1
					else:
						hist["16+"] += 1
					if d > STEP:
						over_step += 1
					if d > jump:
						over_jump += 1
	# 1 m moves (8 voxels) that rise more than a jump: a cliff or ledge the player must climb or go around
	var walls := 0
	var moves := 0
	var s := int(Settings.m2v(1.0))
	for j in range(-r, r + 1, s):
		for i in range(-r, r + 1 - s, s):
			var a := gen.height_at(cx + i, cz + j)
			var b := gen.height_at(cx + i + s, cz + j)
			if a <= sea + 2 and b <= sea + 2:
				continue
			moves += 1
			if absi(b - a) > jump:
				walls += 1
	return {"at": [cx, cz], "pairs": n, "hist": hist, "single_voxel_share": snappedf(float(ones) / maxf(1.0, float(rises)), 0.001),
		"over_step_share": snappedf(float(over_step) / maxf(1.0, float(n)), 0.0001), "over_jump_share": snappedf(float(over_jump) / maxf(1.0, float(n)), 0.0001),
		"wall_share_1m": snappedf(float(walls) / maxf(1.0, float(moves)), 0.0001)}


func _ocean(gen: FineGen, cx: int, cz: int, km: float) -> Dictionary:
	var g: WorldGen = gen.get_macro()
	var sea_m := Settings.SEA_LEVEL_VOXEL / FineGen.FS
	var step := 64   # macro columns (32 m)
	var half := int(km * 1000.0 / 0.5 / 2.0)
	var mx0 := cx / FineGen.FS
	var mz0 := cz / FineGen.FS
	var depths := []
	for j in range(-half, half + 1, step):
		for i in range(-half, half + 1, step):
			var d: Dictionary = g.column_info(mx0 + i, mz0 + j)
			if bool(d["lake"]) or float(d["continentalness"]) > -0.05:
				continue
			var h := int(d["height"])
			if h < sea_m:
				depths.append((sea_m - h) * 0.5)
	depths.sort()
	var n := depths.size()
	var here: Dictionary = g.column_info(mx0, mz0)
	if n == 0:
		return {"columns": 0}
	var deep := func(m: float) -> float: return snappedf(float(depths.filter(func(v): return v > m).size()) / n, 0.001)
	return {"columns": n, "p25_m": depths[n / 4], "p50_m": depths[n / 2], "p90_m": depths[n * 9 / 10], "max_m": depths[n - 1],
		"over_20m": deep.call(20.0), "over_50m": deep.call(50.0), "over_80m": deep.call(80.0),
		"report_spot_m": (sea_m - int(here["height"])) * 0.5, "report_c": snappedf(float(here["continentalness"]), 0.001)}


func _trees(gen: FineGen, count: int, kind: String) -> Dictionary:
	var wood := {}
	for m in range(1, 512):
		var nm := GameMaterials.name_of(m)
		if nm.ends_with("_log") or nm.ends_with("_bark") or nm == "wood":
			wood[m] = true
	var trunks := 0
	var floating := 0
	var found := []
	var profiles := []
	var start := Vector2i(0, 0)
	for k in count:
		var p: Vector3i = gen.find_nearest(kind, start.x, start.y, int(Settings.m2v(3000.0)))
		if p.y < 0:
			break
		found.append([p.x, p.z])
		if kind == "ravine":
			# across the ravine: of the two axes, the one where it is narrower
			var px := _ravine_profile(gen, p)
			var pz := _ravine_profile(gen, p, true)
			profiles.append(pz if float(pz.get("top_width_m", 1e9)) < float(px.get("top_width_m", 1e9)) else px)
		start = Vector2i(p.x + int(Settings.m2v(900.0)), p.z + int(Settings.m2v(700.0)))
		# 96 x 96 columns around it, in 16-column tiles from just under the uncarved ground to 24 m over it
		for tz in range(-48, 48, 16):
			for tx in range(-48, 48, 16):
				var ox := p.x + tx
				var oz := p.z + tz
				var hmin := 1 << 30
				var hmax := -1
				for c in [Vector2i(0, 0), Vector2i(15, 0), Vector2i(0, 15), Vector2i(15, 15), Vector2i(8, 8)]:
					var hh := gen.height_at(ox + c.x, oz + c.y)
					hmin = mini(hmin, hh)
					hmax = maxi(hmax, hh)
				var y0 := (hmin - 24) & ~15
				var y1 := hmax + int(Settings.m2v(24.0))
				var vox := {}
				for by in range(y0, y1, 16):
					var bytes: PackedByteArray = gen.generate_block(Vector3i(ox, by, oz), Vector3i(16, 16, 16), 0, 2)
					if bytes.is_empty():
						continue
					for i in 4096:
						var m := bytes.decode_u16(i * 2)
						if m != 0:
							vox[Vector3i(ox + (i / 16) % 16, by + i % 16, oz + i / 256)] = m
				for lz in 16:
					for lx in 16:
						# the lowest wood voxel of the column with a run of 8+ wood above it: a trunk foot
						for y in range(y0, y1):
							var q := Vector3i(ox + lx, y, oz + lz)
							if not wood.has(vox.get(q, 0)):
								continue
							var run := 0
							while run < 8 and wood.has(vox.get(q + Vector3i(0, run, 0), 0)):
								run += 1
							if run >= 8:
								trunks += 1
								if vox.get(q - Vector3i(0, 1, 0), 0) == 0 and vox.get(q - Vector3i(0, 2, 0), 0) == 0:
									floating += 1
							break
	return {"ravines": found, "trunk_columns": trunks, "floating_trunk_columns": floating, "profiles": profiles}
