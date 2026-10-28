extends SceneTree
## Run: Godot_console.exe --headless --path game --script res://tools/ore_mass_probe.gd (2026-10-28: 1.86 M ore, 75 % massed; before the bunching 1.76 M, 59 %)
## Ore in eight buried veins, sampled 10 m out from each centre (the outer lens, where strays lie):
## total ore voxels, and the share of it in 1 m cubes that are at least 40 % ore (solid masses).
func _init() -> void:
	var S := preload("res://scripts/settings.gd")
	var gen := FineGen.new()
	gen.configure(1337, S.SEA_LEVEL_VOXEL, S.WORLD_HEIGHT_VOXELS)
	var F := FineGen.FS
	var ores := [16, 17, 18, 19]
	var c := 192 * F
	var total := 0
	var massed := 0
	var n := 0
	for cz in range(-6, 6):
		for cx in range(-6, 6):
			if n >= 8:
				break
			var v: Dictionary = gen.vein_at(cx * c + c / 2, cz * c + c / 2)
			if v.is_empty() or gen.height_at(int(v["x"]), int(v["z"])) - int(v["y"]) < 40 * F:
				continue
			n += 1
			var t: int = (int(v["thickness"]) * F / 8) * 8
			var sz := Vector3i(64, t, 64)
			for off in [Vector2i(80, 0), Vector2i(-80, 0), Vector2i(0, 80), Vector2i(0, -80)]:
				var b: PackedByteArray = gen.generate_block(Vector3i(int(v["x"]) - 32 + off.x, int(v["y"]) - t / 2, int(v["z"]) - 32 + off.y), sz, 0, 2)
				var cells := {}
				for i in sz.x * sz.y * sz.z:
					if b.decode_u16(i * 2) in ores:
						total += 1
						var k := Vector3i((i / sz.y) % sz.x / 8, i % sz.y / 8, i / (sz.y * sz.x) / 8)
						cells[k] = cells.get(k, 0) + 1
				for k in cells:
					if cells[k] >= 205:
						massed += cells[k]
	print("ORE veins=%d total=%d massed=%.2f" % [n, total, float(massed) / maxf(1.0, total)])
	quit()
