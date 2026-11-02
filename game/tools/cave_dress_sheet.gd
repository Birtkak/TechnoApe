extends SceneTree

## Debug: what each cave biome dresses its floors and ceilings with (FineGen::cave_dressing), without a window.
## One row per cave biome: a vertical west-east slice through the nearest spot of it, every voxel in its atlas
## tile's mean colour (cave air black).
##   godot --headless --path game --script res://tools/cave_dress_sheet.gd -- [seed=1337] [out=res://out/cave_dress.png]
##       [biomes=lush_cave,dripstone_cave,...]

const Settings := preload("res://scripts/settings.gd")

const W := 160   # slice width, voxels
const H := 64    # slice height, voxels
const S := 3     # pixels per voxel


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(int(args.get("seed", "1337")), Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var biomes := String(args.get("biomes", "lush_cave,dripstone_cave,fungal_cave,frozen_cave,magma_cave,crystal_cave,sulfur_cave")).split(",")
	var atlas := (load("res://assets/textures/atlas.png") as Texture2D).get_image()
	var mean := {}
	var img := Image.create(W * S, biomes.size() * (H * S + 4), false, Image.FORMAT_RGB8)
	for row in biomes.size():
		var p: Vector3i = gen.find_nearest(biomes[row], 0, 0, int(Settings.m2v(4000.0)))
		if p.y < 0:
			print("no ", biomes[row])
			continue
		var o := Vector3i(p.x - W / 2, p.y - H / 2, p.z)
		var b: PackedByteArray = gen.generate_block(o, Vector3i(W, H, 1), 0, 2)
		for x in W:
			for y in H:
				var m := 0 if b.size() < 2 else (b.decode_u16(0) if b.size() == 2 else b.decode_u16(2 * (y + H * x)))
				if not mean.has(m):
					mean[m] = _tile_mean(atlas, m)
				img.fill_rect(Rect2i(x * S, row * (H * S + 4) + (H - 1 - y) * S, S, S), mean[m])
		# a camera line for a look check: along the longest run of air in the slice, a metre over its floor
		var best := [0, 0, 0]   # length, start x, y
		for y in range(4, H - 4):
			var run := 0
			for x in W:
				var m := 0 if b.size() <= 2 else b.decode_u16(2 * (y + H * x))
				run = run + 1 if m == 0 else 0
				if run > best[0]:
					best = [run, x - run + 1, y]
		var cy: int = o.y + best[2]
		print(biomes[row], " at ", p, " cam ", Vector3i(o.x + best[1] + 1, cy + 4, p.z), " look ", Vector3i(o.x + best[1] + best[0] - 1, cy - 2, p.z))
	var out := String(args.get("out", "res://out/cave_dress.png"))
	img.save_png(out)
	print("wrote ", ProjectSettings.globalize_path(out))
	quit(0)


func _tile_mean(atlas: Image, m: int) -> Color:
	if m == 0:
		return Color(0.02, 0.02, 0.03)
	var t: int = GameMaterials.defs()[m - 1]["tiles"][1]
	var tx := (t % GameMaterials.ATLAS_TILES.x) * 16
	var ty := (t / GameMaterials.ATLAS_TILES.x) * 16
	var c := Color(0, 0, 0)
	for y in 16:
		for x in 16:
			c += atlas.get_pixel(tx + x, ty + y)
	return c / 256.0
