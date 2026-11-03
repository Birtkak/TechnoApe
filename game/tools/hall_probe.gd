extends SceneTree

## Debug: are the cave halls walkable (ADR 0134)? Samples the generated caves (LOD 0) on a 1 m grid of columns
## in a square round (x, z), from the ground down depth_m, and prints one JSON line:
##   halls: open spaces at least 4 m tall and 36 m^2, one per storey (runs of the next column joined where they share
##     2 m of height): across (sqrt of the floor area) and tall as p50 / p90 / max, the largest with bbox and spot
##   drops: stepping from a floor cell to the next column (1 m over) where the body fits, how far down its floor
##     lies: counts at or past 2 / 4 / 10 / 20 m, the worst with its spot; into_hall the same for steps from
##     outside a hall (a passage, a ledge) onto a hall floor open 6 m to every side; into_shaft the steps into the
##     narrow part of a tall space (a shaft, a hall's tapering end)
##   floor: hall floor cells that are rubble (gravel, cobble); from up to 300 sampled 1 m squares of hall floor (cave
##     dressing is only generated in short blocks): glow voxels (glow mushrooms, glowing caps, crystal, magma crust) in
##     the first 2 m over the floor per 100 m^2, the share of squares with any, and moss voxels on the roof above
##   godot --headless --path game --script res://tools/hall_probe.gd -- seed=1932036364 x=974 z=-1191 [r_m=48]
##       [depth_m=110]

const Settings := preload("res://scripts/settings.gd")
const STEP := 8      # voxels between sampled columns (1 m)
const HALL := 32     # voxels: a run at least 4 m tall is a hall
const BODY := 14     # voxels of headroom a step needs (1.75 m)
const WIDE := 36.0   # m^2: a blob of tall runs at least this big is a hall, not a shaft


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(int(args.get("seed", "1932036364")), Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var cx := int(args.get("x", "974"))
	var cz := int(args.get("z", "-1191"))
	var n := int(Settings.m2v(float(args.get("r_m", "48")))) * 2 / STEP
	var depth_v := int(Settings.m2v(float(args.get("depth_m", "110"))))
	var x0 := cx - n * STEP / 2
	var z0 := cz - n * STEP / 2
	var top := 0
	for j in range(0, n, 8):
		for i in range(0, n, 8):
			top = maxi(top, gen.height_at(x0 + i * STEP, z0 + j * STEP))
	top = (top >> 4 << 4) + 16
	var y0 := maxi(32, top - depth_v)
	var h := top - y0
	var rubble := {}
	var glow := {}
	for m in range(1, 512):
		var nm := GameMaterials.name_of(m)
		if nm.contains("gravel") or nm.contains("cobble") or nm.contains("rubble"):
			rubble[m] = true
		if nm.contains("glow") or nm == "crystal" or nm == "fungus_cap" or nm == "magma_rock":
			glow[m] = true
	# runs[col] = [floor, top, floor material, glow count over the floor, ...] for each cave air run under the ground
	var runs := []
	runs.resize(n * n)
	for j in n:
		var bytes: PackedByteArray = gen.generate_block(Vector3i(x0, y0, z0 + j * STEP), Vector3i(n * STEP, h, 1), 0, 2)
		for i in n:
			var col := PackedInt32Array()
			var ground := gen.height_at(x0 + i * STEP, z0 + j * STEP) - y0 - 6
			var base := i * STEP * h
			var y := 1
			while y < mini(h, ground):
				if bytes.is_empty() or bytes.decode_u16((base + y) * 2) != 0:
					y += 1
					continue
				var f := y
				var g := 0
				var below := 0 if bytes.is_empty() else bytes.decode_u16((base + f - 1) * 2)
				while y < h and (bytes.is_empty() or bytes.decode_u16((base + y) * 2) == 0 or glow.has(bytes.decode_u16((base + y) * 2))):
					if y - f < 16 and not bytes.is_empty() and glow.has(bytes.decode_u16((base + y) * 2)):
						g += 1
					y += 1
				if y < ground:   # under the ground: a cave run
					col.append_array([f, y, below, g])
			runs[j * n + i] = col
	# halls: tall runs (at least HALL) joined to the next column's tall run where they share 2 m of height, so the
	# halls of two storeys stay apart; a blob of at least WIDE m^2 is a hall (a shaft is a few m^2)
	var node := {}   # Vector2i(column, run offset) -> blob id
	var blobs := []
	for k0 in n * n:
		var c0: PackedInt32Array = runs[k0]
		for r0 in range(0, c0.size(), 4):
			if c0[r0 + 1] - c0[r0] < HALL or node.has(Vector2i(k0, r0)):
				continue
			var id := blobs.size()
			var b := {"area": 0, "tall": 0, "runs": [], "min": Vector2i(k0 % n, k0 / n), "max": Vector2i(k0 % n, k0 / n)}
			node[Vector2i(k0, r0)] = id
			var stack := [Vector2i(k0, r0)]
			while not stack.is_empty():
				var p: Vector2i = stack.pop_back()
				var c: PackedInt32Array = runs[p.x]
				b["area"] += 1
				b["runs"].append(c[p.y + 1] - c[p.y])
				if c[p.y + 1] - c[p.y] > b["tall"]:
					b["tall"] = c[p.y + 1] - c[p.y]
					b["tall_at"] = [x0 + p.x % n * STEP, y0 + c[p.y], z0 + p.x / n * STEP]
				b["min"] = Vector2i(mini(b["min"].x, p.x % n), mini(b["min"].y, p.x / n))
				b["max"] = Vector2i(maxi(b["max"].x, p.x % n), maxi(b["max"].y, p.x / n))
				for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
					var ii: int = p.x % n + d.x
					var jj: int = p.x / n + d.y
					if ii < 0 or jj < 0 or ii >= n or jj >= n:
						continue
					var kk := jj * n + ii
					var nc: PackedInt32Array = runs[kk]
					for q in range(0, nc.size(), 4):
						if nc[q + 1] - nc[q] < HALL or node.has(Vector2i(kk, q)):
							continue
						if mini(nc[q + 1], c[p.y + 1]) - maxi(nc[q], c[p.y]) < 16:
							continue
						node[Vector2i(kk, q)] = id
						stack.append(Vector2i(kk, q))
			blobs.append(b)
	var halls := []
	for b in blobs:
		var m2: float = b["area"] * STEP * STEP * Settings.VOXEL_M * Settings.VOXEL_M
		if m2 < WIDE:
			continue
		b["runs"].sort()
		# tall_m: the p90 run, a hall's height without a shaft that opens into it (tallest_m)
		halls.append({"across_m": snappedf(sqrt(m2), 0.1), "tall_m": b["runs"][int(b["runs"].size() * 0.9)] * Settings.VOXEL_M,
			"tallest_m": b["tall"] * Settings.VOXEL_M, "tall_at": b["tall_at"],
			"bbox_m": [(b["max"].x - b["min"].x + 1) * STEP * Settings.VOXEL_M, (b["max"].y - b["min"].y + 1) * STEP * Settings.VOXEL_M],
			"at": [x0 + (b["min"].x + b["max"].x) / 2 * STEP, z0 + (b["min"].y + b["max"].y) / 2 * STEP]})
	halls.sort_custom(func(a, b): return a["across_m"] > b["across_m"])
	var in_hall := func(k: int, r: int) -> bool:
		var id = node.get(Vector2i(k, r), -1)
		return id >= 0 and blobs[id]["area"] * STEP * STEP * Settings.VOXEL_M * Settings.VOXEL_M >= WIDE
	# open 6 m to all four sides at the run's height: a hall floor, not the inside of a shaft
	var wide := func(k: int, r: int) -> bool:
		var c: PackedInt32Array = runs[k]
		for d in [Vector2i(6, 0), Vector2i(-6, 0), Vector2i(0, 6), Vector2i(0, -6)]:
			var ii: int = k % n + d.x
			var jj: int = k / n + d.y
			if ii < 0 or jj < 0 or ii >= n or jj >= n:
				return false
			var nc: PackedInt32Array = runs[jj * n + ii]
			var ok := false
			for q in range(0, nc.size(), 4):
				if nc[q + 1] - nc[q] >= HALL and mini(nc[q + 1], c[r + 1]) - maxi(nc[q], c[r]) >= 16:
					ok = true
			if not ok:
				return false
		return true
	# drops and floors
	var steps := 0
	var cnt := {"2m": 0, "4m": 0, "10m": 0, "20m": 0}
	var worst := {"m": 0.0}
	var into := []
	var worst_into := {"m": 0.0}
	var shaft := []
	var floor_cells := 0
	var rubble_cells := 0
	var glow_n := 0
	var samples := []   # hall floor cells for the dressing sample: [x, floor y, z, roof y]
	for j in n:
		for i in n:
			var k := j * n + i
			var col: PackedInt32Array = runs[k]
			for r in range(0, col.size(), 4):
				var f := col[r]
				var t := col[r + 1]
				if t - f < BODY:
					continue
				var hall: bool = in_hall.call(k, r)
				if hall:
					floor_cells += 1
					if rubble.has(col[r + 2]):
						rubble_cells += 1
					glow_n += col[r + 3]
					if (i * 7 + j * 3) % 11 == 0:
						samples.append([x0 + i * STEP, y0 + f, z0 + j * STEP, y0 + t])
				for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
					var ii: int = i + d.x
					var jj: int = j + d.y
					if ii < 0 or jj < 0 or ii >= n or jj >= n:
						continue
					var kk := jj * n + ii
					var nc: PackedInt32Array = runs[kk]
					for q in range(0, nc.size(), 4):
						if nc[q] > f or nc[q + 1] < f + BODY:
							continue
						steps += 1
						var drop := (f - nc[q]) * Settings.VOXEL_M
						for key in [2, 4, 10, 20]:
							if drop >= key:
								cnt["%dm" % key] += 1
						var spot := {"m": drop, "from": [x0 + i * STEP, y0 + f, z0 + j * STEP], "to_floor": y0 + nc[q]}
						if drop > worst["m"]:
							worst = spot
						if not hall and in_hall.call(kk, q):   # from a passage or a ledge into a hall
							if wide.call(kk, q):
								into.append(drop)
								if drop > worst_into["m"]:
									worst_into = spot
							else:
								shaft.append(drop)
	into.sort()
	# dressing (glow mushrooms, moss) is only generated in blocks up to 64 voxels tall: 1 m squares round sampled hall
	# floor cells, the first 2 m over the floor, and the roof's last metre
	var moss := GameMaterials.id_of("moss")
	var glow_v := 0
	var glow_cells := 0
	var moss_v := 0
	samples.shuffle()
	samples = samples.slice(0, 300)
	for sm in samples:
		var fb: PackedByteArray = gen.generate_block(Vector3i(sm[0] - 4, sm[1] - 1, sm[2] - 4), Vector3i(8, 16, 8), 0, 2)
		var any_glow := false
		for q in range(0, fb.size(), 2):
			if glow.has(fb.decode_u16(q)):
				glow_v += 1
				any_glow = true
		if any_glow:
			glow_cells += 1
		var cb: PackedByteArray = gen.generate_block(Vector3i(sm[0] - 4, sm[3] - 8, sm[2] - 4), Vector3i(8, 10, 8), 0, 2)
		for q in range(0, cb.size(), 2):
			if cb.decode_u16(q) == moss:
				moss_v += 1
	var pct := func(a: Array, p: float) -> float: return 0.0 if a.is_empty() else a[mini(a.size() - 1, int(a.size() * p))]
	var area_100 := floor_cells * STEP * STEP * Settings.VOXEL_M * Settings.VOXEL_M / 100.0
	var ac := _sorted(halls, "across_m")
	var tl := _sorted(halls, "tall_m")
	print(JSON.stringify({"box": {"x0": x0, "z0": z0, "n": n, "y0": y0, "top": top},
		"halls": {"count": halls.size(), "largest": halls.slice(0, 6),
			"across_m": [pct.call(ac, 0.5), pct.call(ac, 0.9), ac.back() if not ac.is_empty() else 0.0],
			"tall_m": [pct.call(tl, 0.5), pct.call(tl, 0.9), tl.back() if not tl.is_empty() else 0.0]},
		"drops": {"steps": steps, "at_least": cnt, "worst": worst,
			"into_hall": {"n": into.size(), "p90": pct.call(into, 0.9), "at_least_4m": into.filter(func(v): return v >= 4.0).size(),
				"at_least_10m": into.filter(func(v): return v >= 10.0).size(), "worst": worst_into},
			"into_shaft": {"n": shaft.size(), "at_least_10m": shaft.filter(func(v): return v >= 10.0).size()}},
		"floor": {"hall_floor_m2": snappedf(area_100 * 100.0, 1.0), "rubble_share": snappedf(rubble_cells / maxf(1.0, floor_cells), 0.01),
			"glow_per_100m2": snappedf(glow_v * 100.0 / maxf(1.0, samples.size()), 0.1),
			"glow_m2_share": snappedf(glow_cells / maxf(1.0, samples.size()), 0.01),
			"roof_moss_per_100m2": snappedf(moss_v * 100.0 / maxf(1.0, samples.size()), 0.1), "sampled_m2": samples.size()}}))
	quit(0)


func _sorted(a: Array, key: String) -> Array:
	var v := a.map(func(o): return o[key])
	v.sort()
	return v
