extends SceneTree

## Headless probe (no autoloads: voxels are 0.5 m here) for ADR 0022's wild crops: generates the surface blocks of a
## square around the origin and counts wild wheat / wild carrot plants, groups
## them into patches and reports patches per km^2, plants per patch and the
## nearest patch to the origin (the spawn search starts there). They must be rare.
##   godot_console.exe --headless --path game --script res://tools/wild_crop_probe.gd -- seed=1337 radius=512

var seed_value := 1337
var radius := 512
const B := 16


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "seed":
			seed_value = int(kv[1])
		if kv.size() == 2 and kv[0] == "radius":
			radius = int(kv[1])
	var gen := WorldGen.new()
	gen.configure(seed_value, 256, 768)
	var t0 := Time.get_ticks_usec()
	var found := {GameMaterials.WILD_WHEAT: [], GameMaterials.WILD_CARROT: []}
	var plains_cols := 0
	for bx in range(-radius, radius, B):
		for bz in range(-radius, radius, B):
			var h: int = gen.height_at(bx + 8, bz + 8)
			if gen.biome_at(bx + 8, bz + 8) == WorldGen.BIOME_PLAINS:
				plains_cols += B * B
			var y0 := (h - 4) / B * B
			for oy in [0, B]:
				var data: PackedByteArray = gen.generate_block(Vector3i(bx, y0 + oy, bz), Vector3i(B, B, B), 0, 2)
				if data.is_empty():
					continue
				for i in range(0, data.size(), 2):
					var m := data[i] | (data[i + 1] << 8)
					if found.has(m):
						var k := i / 2
						found[m].append(Vector3i(bx + k % B, y0 + oy + (k / B) % B, bz + k / (B * B)))
	var report := {"test": "wild_crop_probe", "seed": seed_value, "radius_m": radius * 0.5, "ms": (Time.get_ticks_usec() - t0) / 1000,
		"plains_share": snappedf(float(plains_cols) / float(4 * radius * radius), 0.01)}
	var area_km2 := pow(radius * 1.0 / 1000.0, 2)
	for m in found:
		var pts: Array = found[m]
		var patches := []   # [centre sum, count]
		for p in pts:
			var joined := false
			for pa in patches:
				if Vector2(pa[0].x / pa[1], pa[0].z / pa[1]).distance_to(Vector2(p.x, p.z)) < 8.0:
					pa[0] += Vector3(p)
					pa[1] += 1
					joined = true
					break
			if not joined:
				patches.append([Vector3(p), 1])
		var nearest := 1e9
		var sizes := []
		for pa in patches:
			var c: Vector3 = pa[0] / pa[1]
			nearest = minf(nearest, Vector2(c.x, c.z).length() * 0.5)
			sizes.append(pa[1])
			if Vector2(c.x, c.z).length() * 0.5 <= nearest + 0.01:
				report["nearest_" + GameMaterials.name_of(m)] = Vector3i(c.round())
		report[GameMaterials.name_of(m)] = {"plants": pts.size(), "patches": patches.size(),
			"patches_per_km2": snappedf(patches.size() / area_km2, 0.1), "plants_per_patch": sizes,
			"nearest_patch_m": snappedf(nearest, 1.0) if not patches.is_empty() else -1}
	print(JSON.stringify(report))
	quit(0)
