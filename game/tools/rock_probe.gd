extends Node

## Debug: a vertical cut through the world showing the rock kinds and strata (ADR 0057). Generates
## 1 m (LOD 3) slabs along x and paints each voxel with its material's mean atlas colour; air above
## the ground is sky, water blue. Writes a PNG and one JSON line with the share of each rock kind.
##   godot --headless --path game res://tools/rock_probe.tscn -- [seed=1337 z=0 x0=-12000 len_m=3000 out=user://rock_cut.png]

const LOD := 3


func _ready() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(int(args.get("seed", "1337")), Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var step := 1 << LOD
	var z := int(args.get("z", "0"))
	var x0 := int(args.get("x0", "-12000"))
	var cols := int(Settings.m2v(float(args.get("len_m", "3000")))) / step
	var y0 := Settings.SEA_LEVEL_VOXEL - int(Settings.m2v(70.0))
	var rows := int(Settings.m2v(260.0)) / step
	var atlas := Image.load_from_file("res://assets/textures/atlas.png")
	var mean := {}
	var img := Image.create(cols, rows, false, Image.FORMAT_RGB8)
	img.fill(Color(0.62, 0.78, 0.95))
	var counts := {}
	var chunk := 128
	for cx in range(0, cols, chunk):
		var n := mini(chunk, cols - cx)
		var bytes: PackedByteArray = gen.generate_block(Vector3i(x0 + cx * step, y0, z), Vector3i(n, rows, 1), LOD, 2)
		if bytes.is_empty():
			continue
		for x in n:
			for y in rows:
				var id := bytes.decode_u16((y + rows * x) * 2)   # x-major: index = y + sy * (x + sx * z)
				if id == 0:
					continue
				if not mean.has(id):
					mean[id] = _mean(atlas, id)
				img.set_pixel(cx + x, rows - 1 - y, mean[id])
				var name := GameMaterials.name_of(id)
				counts[name] = int(counts.get(name, 0)) + 1
	var out := String(args.get("out", "user://rock_cut.png"))
	img.save_png(out)
	var rocks := {}
	for k in ["stone", "basalt"] + GameMaterials.ROCK_KINDS:
		rocks[k] = counts.get(k, 0)
	print(JSON.stringify({"test": "rock_probe", "out": ProjectSettings.globalize_path(out), "rocks": rocks}))
	get_tree().quit(0)


func _mean(atlas: Image, id: int) -> Color:
	var d: Dictionary = GameMaterials.defs()[id - 1]
	var t := int(d.get("tiles", [0, 0, 0])[1])
	var c := Color(0, 0, 0)
	for y in 16:
		for x in 16:
			c += atlas.get_pixel((t % 8) * 16 + x, (t / 8) * 16 + y)
	return c / 256.0
