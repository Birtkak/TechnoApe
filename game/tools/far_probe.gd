extends SceneTree

## Headless probe of the far layer's data (ADR 0074): builds a few FarMesh tiles around a point and
## prints their sizes, colours and timings.
##   godot --headless --path . --script res://tools/far_probe.gd -- seed=272905368 x=-160 z=-277 level=0

func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var fg := FineGen.new()
	fg.configure(int(args.get("seed", "272905368")), 1024, 5120)   # Settings.SEA_LEVEL_VOXEL, WORLD_HEIGHT_VOXELS (no autoloads here)
	var fm := FarMesh.new()
	fm.configure(fg)
	var colors := FarColors.material_colors()
	fm.set_material_colors(colors)
	for id in [GameMaterials.STONE, GameMaterials.DIRT, GameMaterials.GRASS, GameMaterials.SAND, GameMaterials.LEAVES, GameMaterials.LEAF_LITTER, GameMaterials.SNOW]:
		print("colour %d %s = %s" % [id, GameMaterials.name_of(id), colors[id]])
	var x := int(args.get("x", "-160"))
	var z := int(args.get("z", "-277"))
	for level in range(0, int(args.get("levels", "5"))):
		var size := fm.tile_voxels(level)
		var tx := int(floor(float(x) / size))
		var tz := int(floor(float(z) / size))
		var t0 := Time.get_ticks_usec()
		var d: Dictionary = fm.build_tile(level, tx, tz)
		var ms := (Time.get_ticks_usec() - t0) / 1000.0
		var cols: PackedColorArray = d.get("colors", PackedColorArray())
		var hist := {}
		for c in cols:
			var k := "%.2f,%.2f,%.2f" % [c.r, c.g, c.b]
			hist[k] = int(hist.get(k, 0)) + 1
		var top: Array = hist.keys()
		top.sort_custom(func(a, b): return hist[a] > hist[b])
		var verts: PackedVector3Array = d.get("vertices", PackedVector3Array())
		var ymin := 1e9
		var ymax := -1e9
		for v in verts:
			ymin = minf(ymin, v.y)
			ymax = maxf(ymax, v.y)
		print("level %d tile %d,%d: %.1f ms (native %.1f), verts %d, water verts %d, trees %d, y %.0f..%.0f, colours %s" % [
			level, tx, tz, ms, float(d.get("ms", 0.0)), verts.size(), (d.get("water_vertices", PackedVector3Array()) as PackedVector3Array).size(),
			(d.get("trees", PackedFloat32Array()) as PackedFloat32Array).size() / 7, ymin, ymax, top.slice(0, 4).map(func(k): return "%s x%d" % [k, hist[k]])])
		var cu: PackedByteArray = d.get("custom0", PackedByteArray())
		if cu.size() >= 8:
			print("   custom0 first %s, uv %s, uv2 %s" % [cu.slice(0, 8), (d["uvs"] as PackedVector2Array)[0], (d["uvs2"] as PackedVector2Array)[0]])
	quit(0)
