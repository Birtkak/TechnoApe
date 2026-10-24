extends SceneTree

## Debug (P13 water look): holes and walls in generated water surfaces around a point.
##   godot --headless --path game --script res://tools/water_surface_probe.gd -- seed=2116854772 x=4 z=-1439 [r=64]
## Counts surface cells (water with air above) and, among the air cells at a surface level: holes
## (water on all four sides) and walls (water beside air that has no water under it: a glass edge).
## Prints a few holes with the column under them.

const Settings := preload("res://scripts/settings.gd")


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(int(args.get("seed", "1337")), Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var cx := int(args.get("x", "0"))
	var cz := int(args.get("z", "0"))
	var r := int(args.get("r", "64"))
	var y0 := gen.height_at(cx, cz) - 24
	var size := Vector3i(2 * r, 48, 2 * r)
	var org := Vector3i(cx - r, y0, cz - r)
	var bytes: PackedByteArray = gen.generate_block(org, size, 0, 2)
	var water := {GameMaterials.WATER: true, GameMaterials.WATER_STATIC: true}
	var at := func(x: int, y: int, z: int) -> int:
		if x < 0 or y < 0 or z < 0 or x >= size.x or y >= size.y or z >= size.z:
			return -1
		return bytes.decode_u16((y + size.y * (x + size.x * z)) * 2)
	var surface := 0
	var holes := []
	var walls := 0
	var wall_samples := []
	var depth_hist := {}   # glass edges by how far the water stands above the dry cell's floor (voxels)
	for z in range(1, size.z - 1):
		for x in range(1, size.x - 1):
			for y in range(1, size.y - 1):
				var m: int = at.call(x, y, z)
				if water.has(m) and at.call(x, y + 1, z) == GameMaterials.AIR:
					surface += 1
				if m != GameMaterials.AIR:
					continue
				var wn := 0
				for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
					if water.has(at.call(x + d.x, y, z + d.y)):
						wn += 1
				if wn == 4:
					var col := []
					for k in range(-3, 2):
						col.append(GameMaterials.name_of(at.call(x, y + k, z)))
					holes.append([org + Vector3i(x, y, z), col])
				elif wn > 0 and not water.has(at.call(x, y - 1, z)) and at.call(x, y - 1, z) != GameMaterials.AIR and (y + 1 >= size.y or not water.has(at.call(x, y + 1, z))):
					walls += 1
					# the deepest water beside it: how many water cells stand above this floor
					var deep := 0
					for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
						var k := 0
						while water.has(at.call(x + d.x, y + k, z + d.y)):
							k += 1
						deep = maxi(deep, k)
					depth_hist[deep] = int(depth_hist.get(deep, 0)) + 1
					if deep >= 3 and wall_samples.size() < 6:
						for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
							if water.has(at.call(x + d.x, y, z + d.y)):
								wall_samples.append([org + Vector3i(x, y, z), org + Vector3i(x + d.x, y, z + d.y), deep])
								break
	print("water probe at ", Vector2i(cx, cz), " r ", r, ": surface cells ", surface, ", holes ", holes.size(), ", glass edges ", walls, " by height ", depth_hist)
	for w in wall_samples:
		print("  edge ", w[0], " (", w[2], " high) dry: ", gen.column_info(w[0].x, w[0].z))
		print("      wet ", w[1], ": ", gen.column_info(w[1].x, w[1].z))
	for h in holes.slice(0, 8):
		print("  hole ", h[0], " column y-3..y+1: ", h[1])
	quit(0)
