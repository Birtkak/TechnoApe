extends SceneTree

## Debug: the nearest stone circle (top view and side view) and a slice through the rock round the nearest deep
## cave (ore bright, cave air black), with the ore voxels counted round a few deep caves (ADR 0086).
##   godot --headless --path game --script res://tools/ruin_ore_sheet.gd -- [seed=1337] [out=res://out/ruin_ore.png]
## Prints {"circle": {...}, "ore_voxels": n, "rock_voxels": n} as JSON.

const Settings := preload("res://scripts/settings.gd")

const PX := 3          # image pixels per voxel
const R := 96          # half the square round the circle, voxels (12 m)
const UP := 64         # voxels above the circle's floor (8 m)
const DOWN := 8
const SLICE := 320     # the ore slice's side, voxels (40 m)
const SKY := Color(0.75, 0.83, 0.92)


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(int(args.get("seed", "1337")), Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var defs: Array = GameMaterials.defs()
	var is_ore := func(m: int) -> bool: return m > 0 and m <= defs.size() and String(defs[m - 1]["name"]).ends_with("_ore")
	var color := func(m: int) -> Color:
		if is_ore.call(m):
			return Color.from_hsv(fmod(m * 0.137, 1.0), 0.9, 1.0)
		var h := hash(m)
		return Color(0.35 + (h & 63) / 256.0, 0.35 + ((h >> 6) & 63) / 256.0, 0.35 + ((h >> 12) & 63) / 256.0)
	var sheet := Image.create(2 * R * PX * 2 + SLICE * 2, maxi(2 * R * PX, SLICE * 2), false, Image.FORMAT_RGB8)
	sheet.fill(Color(0.1, 0.1, 0.1))

	# the stone circle: top view (lighter = higher) and side view from -z (darker = further)
	var c: Vector3i = gen.find_nearest("ruin_stone_circle", 0, 0, int(Settings.m2v(4000.0)))
	var info: Dictionary = gen.ruin_at(c.x, c.z)
	var base := c.y
	var o := Vector3i(c.x - R, base - DOWN, c.z - R)
	var n := Vector3i(2 * R, UP + DOWN, 2 * R)
	var b: PackedByteArray = gen.generate_block(o, n, 0, 2)
	var at := func(x: int, y: int, z: int) -> int: return b.decode_u16((y + n.y * (x + n.x * z)) * 2) if not b.is_empty() else 0
	var tallest := 0
	for z in n.z:
		for x in n.x:
			var col := SKY
			for y in range(n.y - 1, -1, -1):
				var m: int = at.call(x, y, z)
				if m != 0:
					col = (color.call(m) as Color) * (0.55 + 0.45 * y / float(n.y))
					break
			sheet.fill_rect(Rect2i(x * PX, z * PX, PX, PX), col)
	for y in n.y:
		for x in n.x:
			var col := SKY
			for z in range(R - 64, R + 64):   # the ring only, not the trees round it
				var m: int = at.call(x, y, z)
				if m != 0:
					col = (color.call(m) as Color) * (1.0 - 0.6 * (z - R + 64) / 128.0)
					if m == 160:   # granite: the stones
						tallest = maxi(tallest, y - DOWN)
					break
			sheet.fill_rect(Rect2i(2 * R * PX + x * PX, (n.y - 1 - y) * PX + (2 * R - n.y) * PX, PX, PX), col)

	# ore: a level slice through the nearest deep cave, and ore counted round four deep caves
	var ore := 0
	var rock := 0
	var x0 := 4 * R * PX
	for k in 4:
		var p: Vector3i = gen.find_nearest("deep_cave", k * 2400, (k % 2) * 2400, int(Settings.m2v(4000.0)))
		var s: PackedByteArray = gen.generate_block(Vector3i(p.x - SLICE / 2, p.y - 32, p.z - SLICE / 2), Vector3i(SLICE, 64, SLICE), 0, 2)
		if s.is_empty():
			continue
		for i in range(0, s.size(), 2):
			var m := s.decode_u16(i)
			if m != 0:
				rock += 1
				if is_ore.call(m):
					ore += 1
		if k == 0:
			for z in SLICE:
				for x in SLICE:
					var m := s.decode_u16((32 + 64 * (x + SLICE * z)) * 2)
					var col: Color = Color(0.02, 0.02, 0.03) if m == 0 else (color.call(m) if is_ore.call(m) else Color(0.45, 0.45, 0.47))
					sheet.fill_rect(Rect2i(x0 + x * 2, z * 2, 2, 2), col)
	var out := String(args.get("out", "res://out/ruin_ore.png"))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(out.get_base_dir()))
	sheet.save_png(out)
	print(JSON.stringify({"out": out, "circle": {"at": [c.x, c.y, c.z], "ruin": info, "tallest_stone_m": tallest * Settings.VOXEL_M},
		"ore_voxels": ore, "rock_voxels": rock, "ore_per_mille": snappedf(1000.0 * ore / maxf(1, rock), 0.01)}))
	quit(0)
