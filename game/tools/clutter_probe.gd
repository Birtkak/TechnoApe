extends SceneTree

## Debug: clutter pieces and ore nuggets as FineGen lays them (ADR 0047). Generates 64 x 64 voxel
## patches at the surface around a grid of points (and the nearest vein), counts the clutter
## materials and measures the pieces GroundPickups makes of them. One JSON line.
##   godot --headless --path game --script res://tools/clutter_probe.gd [-- seed=1337 points=6 spacing_m=300 vein=1]

const Settings := preload("res://scripts/settings.gd")
const N := 64


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(int(args.get("seed", "1337")), Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var points := int(args.get("points", "6"))
	var spacing := Settings.m2v(float(args.get("spacing_m", "300")))
	var watch := ["flower", "flower_red", "cornflower", "oxeye_daisy", "allium", "plant_stem", "mushroom_brown", "mushroom_red",
		"mushroom_stem", "seashell", "kelp", "seagrass", "coal_ore", "iron_ore", "copper_ore", "tin_ore"]
	var centers := []
	for pz in points:
		for px in points:
			var cx := int((px - points / 2) * spacing)
			var cz := int((pz - points / 2) * spacing)
			centers.append(Vector3i(cx, gen.height_at(cx, cz) - 48, cz))
	if args.get("vein", "0") == "1":
		# the veins of a 2 km square instead: a 64 x 96 x 64 box at each
		centers.clear()
		var c := 96 * FineGen.FS
		for vz in range(-6, 6):
			for vx in range(-6, 6):
				var v: Dictionary = gen.vein_at(vx * c + c / 2, vz * c + c / 2)
				if not v.is_empty() and centers.size() < points * points:
					centers.append(Vector3i(int(v["x"]) - N / 2, int(v["y"]) - 48, int(v["z"]) - N / 2))
	var counts := {}
	var sizes := {}   # piece material -> [sizes]
	var t0 := Time.get_ticks_usec()
	var blocks := 0
	for ctr in centers:
		if true:
			var cx: int = ctr.x
			var cz: int = ctr.z
			var vox := {}
			var y0: int = ctr.y
			for bz in range(0, N, 16):
				for bx in range(0, N, 16):
					for by in range(0, 96, 16):
						var o := Vector3i(cx + bx, y0 + by, cz + bz)
						var bytes: PackedByteArray = gen.generate_block(o, Vector3i(16, 16, 16), 0, 2)
						blocks += 1
						if bytes.is_empty():
							continue
						for i in 4096:
							var m := bytes.decode_u16(i * 2)
							if m == 0:
								continue
							var nm := GameMaterials.name_of(m)
							if nm in watch:
								vox[o + Vector3i((i / 16) % 16, i % 16, i / 256)] = m   # godot_voxel order: y fastest, then x, then z
								counts[nm] = int(counts.get(nm, 0)) + 1
			var get_voxel := func(p: Vector3i) -> int: return vox.get(p, 0)
			var seen := {}
			for p in vox:
				if seen.has(p):
					continue
				var pc := GroundPickups.piece(get_voxel, p)
				for q in pc:
					seen[q] = true
				var split := GroundPickups.complete(get_voxel, [p] as Array[Vector3i])
				var nm := GameMaterials.name_of(split["pieces"][0]["material"]) if not split["pieces"].is_empty() else "?"
				if not sizes.has(nm):
					sizes[nm] = []
				sizes[nm].append(pc.size())
	var summary := {}
	for nm in sizes:
		var a: Array = sizes[nm]
		a.sort()
		summary[nm] = {"pieces": a.size(), "min": a[0], "median": a[a.size() / 2], "max": a[-1]}
	print(JSON.stringify({"blocks": blocks, "ms": (Time.get_ticks_usec() - t0) / 1000, "voxels": counts, "pieces": summary}))
	quit(0)
