extends SceneTree

## Debug: the material mix of FineGen blocks around a point (ADR 0033).
##   godot --headless --path game --script res://tools/fine_probe.gd [-- vein=1 | x=.. y=.. z=..]

func _init() -> void:
	var gen := FineGen.new()
	gen.configure(1337, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var p := Vector3i(int(args.get("x", "0")), int(args.get("y", "0")), int(args.get("z", "0")))
	if args.get("vein", "0") == "1":
		var c := 96 * FineGen.FS
		for cz in range(-10, 10):
			for cx in range(-10, 10):
				var v: Dictionary = gen.vein_at(cx * c + c / 2, cz * c + c / 2)
				if not v.is_empty() and v["name"] != "coal" and gen.height_at(int(v["x"]), int(v["z"])) - int(v["y"]) > 40 * FineGen.FS:
					print("vein ", v)
					p = Vector3i(int(v["x"]), int(v["y"]), int(v["z"]))
					break
			if p != Vector3i.ZERO:
				break
	var t0 := Time.get_ticks_usec()
	var bytes: PackedByteArray = gen.generate_block(p - Vector3i(8, 8, 8), Vector3i(16, 16, 16), 0, 2)
	var us := Time.get_ticks_usec() - t0
	var hist := {}
	if not bytes.is_empty():
		for i in 4096:
			var m := bytes.decode_u16(i * 2)
			hist[GameMaterials.name_of(m)] = int(hist.get(GameMaterials.name_of(m), 0)) + 1
	print("block at ", p, " height ", gen.height_at(p.x, p.z), " us ", us, " ", hist)
	print("column ", gen.column_info(p.x, p.z))
	quit(0)
