extends SceneTree

## Headless probe: does the generator place micro-voxel detail where it should?
## Generates LOD 0 blocks over a square around the nearest target of each kind
## (full column height) and counts the detail materials, plus where they stand:
## stalactites must hang under solid rock, stalagmites/pebbles/mushrooms stand on
## solid, lily pads sit on water, reeds come in pairs on marsh ground.
##   godot_console.exe --headless --path game --script res://tools/detail_probe.gd -- seed=1337 radius=96

var seed_value := 1337
var radius := 96
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
	gen.column_info(0, 0)
	var report := {"test": "detail_probe", "seed": seed_value, "radius": radius}
	var names := ["pebbles", "mushroom_brown", "mushroom_red", "reed", "lily_pad", "stalactite", "stalagmite", "stone_boulder_top"]
	for target in ["plains", "forest", "marsh", "highlands"]:
		var c: Vector3i = gen.find_nearest(target, 0, 0, 6000)
		if c.y < 0:
			report[target] = "not found"
			continue
		var cx: int = c.x
		var cz: int = c.z
		var counts := {}
		for n in names:
			counts[n] = 0
		var bad := {"stalactite_no_ceiling": 0, "floor_detail_floating": 0, "lily_not_on_water": 0, "reed_single": 0}
		var blocks := 0
		var t0 := Time.get_ticks_usec()
		var x0 := (cx - radius) / B * B
		var z0 := (cz - radius) / B * B
		for bz in range(z0, cz + radius, B):
			for bx in range(x0, cx + radius, B):
				# whole column in 16-high blocks from 128 to 400 (caves + surface)
				var col := PackedInt32Array()
				col.resize(272 * B * B)
				var any := false
				for by in range(128, 400, B):
					var bytes: PackedByteArray = gen.generate_block(Vector3i(bx, by, bz), Vector3i(B, B, B), 0, 2)
					blocks += 1
					if bytes.is_empty():
						continue
					any = true
					for i in B * B * B:
						var y := i % B
						var x := (i / B) % B
						var z := i / (B * B)
						var m := bytes.decode_u16(i * 2)
						col[(by - 128 + y) + 272 * (x + B * z)] = m
				if not any:
					continue
				for x in B:
					for z in B:
						var base := 272 * (x + B * z)
						for y in range(1, 271):
							var m := col[base + y]
							var below := col[base + y - 1]
							var above := col[base + y + 1]
							match m:
								GameMaterials.PEBBLES, GameMaterials.MUSHROOM_BROWN, GameMaterials.MUSHROOM_RED, GameMaterials.STALAGMITE:
									counts[GameMaterials.name_of(m)] += 1
									if below == GameMaterials.AIR or below == GameMaterials.WATER or below == GameMaterials.WATER_STATIC:
										bad["floor_detail_floating"] += 1
								GameMaterials.STALACTITE:
									counts["stalactite"] += 1
									if above == GameMaterials.AIR:
										bad["stalactite_no_ceiling"] += 1
								GameMaterials.LILY_PAD:
									counts["lily_pad"] += 1
									if below != GameMaterials.WATER_STATIC and below != GameMaterials.WATER:
										bad["lily_not_on_water"] += 1
								GameMaterials.REED:
									counts["reed"] += 1
									if below != GameMaterials.REED and above != GameMaterials.REED:
										bad["reed_single"] += 1
								GameMaterials.STONE:
									if below != GameMaterials.STONE and above == GameMaterials.AIR and (below == GameMaterials.GRASS or below == GameMaterials.DIRT or below == GameMaterials.GRAVEL_STATIC):
										counts["stone_boulder_top"] += 1
		var ms := (Time.get_ticks_usec() - t0) / 1000.0
		report[target] = {"at": [cx, cz], "blocks": blocks, "gen_ms": snappedf(ms, 1), "counts": counts, "bad": bad}
	print(JSON.stringify(report))
	quit(0)
