extends SceneTree

## Debug: exposed voxel faces per material over a patch of the world at one LOD (what the mesher
## turns into triangles: 2 per face), to find what makes a biome expensive to draw (ADR 0033).
##   godot --headless --path game --script res://tools/face_probe.gd -- [biome=forest] [lod=0] [m=32]

const Settings := preload("res://scripts/settings.gd")


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(1337, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var p: Vector3i = gen.find_nearest(args.get("biome", "forest"), 0, 0, int(Settings.m2v(8000.0)))
	var lod := int(args.get("lod", "0"))
	var step := 1 << lod
	var n := int(Settings.m2v(float(args.get("m", "32")))) / step / 16   # blocks per side
	var faces := {}
	var total := 0
	for bz in n:
		for bx in n:
			var x := p.x + (bx - n / 2) * 16 * step
			var z := p.z + (bz - n / 2) * 16 * step
			var h := gen.height_at(x + 8 * step, z + 8 * step)
			for by in range(-1, 4):
				var o := Vector3i(x, (floori(float(h) / (16 * step)) + by) * 16 * step, z)
				var b: PackedByteArray = gen.generate_block(o, Vector3i(16, 16, 16), lod, 2)
				if b.is_empty():
					continue
				for iz in 16:
					for ix in 16:
						for iy in 16:
							var m := b.decode_u16(((iz * 16 + ix) * 16 + iy) * 2)
							if m == 0:
								continue
							var f := 0
							for d in [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 1, 0), Vector3i(0, -1, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]:
								var q: Vector3i = Vector3i(ix, iy, iz) + d
								if q.x < 0 or q.y < 0 or q.z < 0 or q.x > 15 or q.y > 15 or q.z > 15:
									continue   # block borders: unknown, not counted
								if b.decode_u16(((q.z * 16 + q.x) * 16 + q.y) * 2) == 0:
									f += 1
							if f > 0:
								var nm := GameMaterials.name_of(m)
								faces[nm] = int(faces.get(nm, 0)) + f
								total += f
	var rows := []
	for k in faces:
		rows.append([faces[k], k])
	rows.sort()
	rows.reverse()
	var area := float(n * 16 * step) * Settings.VOXEL_M
	print(JSON.stringify({"at": p, "lod": lod, "area_m": area, "faces": total, "faces_per_m2": snappedf(total / (area * area), 0.1), "top": rows.slice(0, 12)}))
	quit(0)
