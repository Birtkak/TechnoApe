extends SceneTree

## Debug (P13 water look): holes and walls in generated water surfaces around a point.
##   godot --headless --path game --script res://tools/water_surface_probe.gd -- seed=2116854772 x=4 z=-1439 [r=64] [y0= hgt=48]
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
	# y0= and hgt= fix the window (to compare generator versions whose ground differs at the centre)
	var y0 := int(args.get("y0", str(gen.height_at(cx, cz) - 24)))
	var size := Vector3i(2 * r, int(args.get("hgt", "48")), 2 * r)
	var org := Vector3i(cx - r, y0, cz - r)
	var bytes: PackedByteArray = gen.generate_block(org, size, 0, 2)
	# every liquid counts as the surface (liquid=water keeps the old water-only count); still kinds included
	var water := {GameMaterials.WATER: true, GameMaterials.WATER_STATIC: true}
	if String(args.get("liquid", "all")) == "all":
		for m in [GameMaterials.LAVA, GameMaterials.LAVA_STATIC, GameMaterials.TOXIC_SLUDGE, GameMaterials.TOXIC_SLUDGE_STATIC]:
			water[m] = true
	var at := func(x: int, y: int, z: int) -> int:
		if x < 0 or y < 0 or z < 0 or x >= size.x or y >= size.y or z >= size.z:
			return -1
		return bytes.decode_u16((y + size.y * (x + size.x * z)) * 2)
	var surface := 0
	var lone := 0   # surface cells with no liquid on any side and open air beside them (ADR 0069 C: the generator drops them)
	var boxed := 0   # ... with rock on all four sides: a one-cell puddle in a one-cell hollow, allowed
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
					var side := 0
					var open := 0
					for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
						var nm: int = at.call(x + d.x, y, z + d.y)
						if water.has(nm):
							side += 1
						elif nm == GameMaterials.AIR:
							open += 1
					if side == 0:
						if open > 0:
							lone += 1
						else:
							boxed += 1
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
	# Seams (ADR 0069 part A): the mesher's surface rule mirrored here, so a corner shared by neighbouring surface
	# cells can be checked for agreement. rule=field (default): a corner is the mean of the surface tops of the
	# columns around it, each column's highest water within two cells up or down (voxel_patch_fluid.py). rule=ramps: the ADR 0045 rule (a corner next to open air drops to
	# the cell's bottom). A seam is two adjacent surface cells, at most two voxels apart in height, that give the
	# corner they share different heights: every seam is a wedge on screen. Lips (adjacent surface cells three or
	# more voxels apart, beyond the mesher's two-cell reach) are a drop, not a seam, and are counted apart; so are wall
	# corners, where a column at the corner holds water three or more cells above the lower cell (a generator terrace,
	# drawn as a vertical water face).
	var rule := String(args.get("rule", "field"))
	const TOP := 0.9375   # a full cell's surface (BakedFluid::TOP_HEIGHT); generated water is always full
	var corners := {}   # Vector2i corner point -> [[cell y, height], ...]
	var col_top := func(x: int, z: int, yref: int) -> Array:   # [votes, world height] by the field rule
		for dy in [2, 1, 0, -1, -2]:   # the column's highest water within two cells of the viewer
			if water.has(at.call(x, yref + dy, z)):
				return [true, yref + dy + TOP]
		if at.call(x, yref, z) == GameMaterials.AIR:
			return [true, -1.0]   # open, nothing in reach: votes the bottom of the lowest water cell at the corner
		return [false, 0.0]
	for z in range(1, size.z - 1):
		for x in range(1, size.x - 1):
			for y in range(1, size.y - 1):
				if not water.has(at.call(x, y, z)) or at.call(x, y + 1, z) != GameMaterials.AIR:
					continue
				for oz in [0, 1]:
					for ox in [0, 1]:
						var h := 0.0
						if rule == "field":
							var sum := 0.0
							var n := 0
							var dry := 0
							var low := 1e9
							for dz in [-1, 0]:
								for dx in [-1, 0]:
									var v: Array = col_top.call(x + ox + dx, z + oz + dz, y)
									if not v[0]:
										continue
									if v[1] < 0.0:
										dry += 1
									else:
										sum += v[1]
										low = minf(low, floorf(v[1]))
										n += 1
							h = (sum + dry * low) / (n + dry)
						else:
							# ramps: the corner drops when an open cell sits side by side with water around it
							h = y + TOP
							var side_a: int = at.call(x + (1 if ox == 1 else -1), y, z)
							var side_b: int = at.call(x, y, z + (1 if oz == 1 else -1))
							var diag: int = at.call(x + (1 if ox == 1 else -1), y, z + (1 if oz == 1 else -1))
							var covered := func(px: int, pz: int) -> bool:
								return water.has(at.call(px, y, pz)) and water.has(at.call(px, y + 1, pz))
							var open_a := side_a == GameMaterials.AIR
							var open_b := side_b == GameMaterials.AIR
							var open_d := diag == GameMaterials.AIR
							if open_a or open_b or (open_d and (water.has(side_a) or water.has(side_b))):
								h = float(y)
							elif covered.call(x + (1 if ox == 1 else -1), z) or covered.call(x, z + (1 if oz == 1 else -1)):
								h = y + 1.0
						var key := Vector2i(x + ox, z + oz)
						if not corners.has(key):
							corners[key] = []
						corners[key].append([y, h])
	var steps := {}   # height difference between horizontally adjacent surface cells (0 = level)
	var tops := {}
	for z in range(1, size.z - 1):
		for x in range(1, size.x - 1):
			for y in range(1, size.y - 1):
				if water.has(at.call(x, y, z)) and at.call(x, y + 1, z) == GameMaterials.AIR:
					tops[Vector2i(x, z)] = y
	# a step of three or more between neighbouring surface cells is a cascade when the bed under them drops as much
	# (the river runs down a slope steeper than 2:1: the ground's doing) and a terrace otherwise (a generator fault)
	var cascades := 0
	var terraces := 0
	var terrace_samples := []
	var bed_at := func(x: int, z: int, y: int) -> int:
		var b := y
		while b > 0 and water.has(at.call(x, b, z)):
			b -= 1
		return b
	for k2 in tops:
		for d in [Vector2i(1, 0), Vector2i(0, 1)]:
			if tops.has(k2 + d):
				var dy: int = absi(tops[k2] - tops[k2 + d])
				steps[dy] = int(steps.get(dy, 0)) + 1
				if dy >= 3:
					var b1: int = bed_at.call(k2.x, k2.y, tops[k2])
					var b2: int = bed_at.call(k2.x + d.x, k2.y + d.y, tops[k2 + d])
					if absi(b1 - b2) >= dy - 1:
						cascades += 1
					else:
						terraces += 1
						if terrace_samples.size() < 3:
							terrace_samples.append([org + Vector3i(k2.x, tops[k2], k2.y), dy, b1 - b2])
	var seams := 0
	var lips := 0
	var wall_corners := 0   # a column at the corner that the two cells see differently (out of reach): a terrace or rapid
	var seam_samples := []
	for key in corners:
		var list: Array = corners[key]
		for a in range(list.size()):
			for b in range(a + 1, list.size()):
				var dy: int = absi(list[a][0] - list[b][0])
				if dy >= 3:
					lips += 1
				elif absf(list[a][1] - list[b][1]) > 0.001:
					# out of reach: a column at the corner that the two cells see differently (its top more than two
					# cells from one of them): a terrace or rapid the mesher draws as a vertical face, not a seam
					var wall := false
					for dz in [-1, 0]:
						for dx in [-1, 0]:
							var va: Array = col_top.call(key.x + dx, key.y + dz, list[a][0])
							var vb: Array = col_top.call(key.x + dx, key.y + dz, list[b][0])
							if va[0] != vb[0] or (va[0] and absf(va[1] - vb[1]) > 0.001):
								wall = true
					if wall:
						wall_corners += 1
						continue
					seams += 1
					if seam_samples.size() < 4:
						# the four columns around the corner, materials from two below the lower cell to two above the upper
						var ya: int = mini(list[a][0], list[b][0])
						var cols := []
						for dz in [-1, 0]:
							for dx in [-1, 0]:
								var stack := []
								for yy in range(ya - 2, ya + 5):
									stack.append(GameMaterials.name_of(at.call(key.x + dx, yy, key.y + dz)).left(5))
								cols.append(stack)
						seam_samples.append([org + Vector3i(key.x, list[a][0], key.y), list[a][1], list[b][0] - list[a][0], list[b][1], cols])
	print("water probe at ", Vector2i(cx, cz), " r ", r, ": surface cells ", surface, ", lone cells ", lone, " (boxed one-cell puddles ", boxed, "), holes ", holes.size(), ", glass edges ", walls, " by height ", depth_hist)
	print("  rule ", rule, ": corner seams ", seams, ", wall corners ", wall_corners, ", lips ", lips, " (", corners.size(), " corner points); surface steps by height ", steps)
	print("  steps of 3+: cascades (the bed drops as much) ", cascades, ", terraces (it does not) ", terraces)
	for ts in terrace_samples:
		print("  terrace at ", ts[0], ": step ", ts[1], ", bed difference ", ts[2])
	for sm in seam_samples:
		print("  seam at corner ", sm[0], ": ", snappedf(sm[1], 0.001), " vs ", snappedf(sm[3], 0.001), " (dy ", sm[2], ") columns (-x-z, +x-z, -x+z, +x+z) from y-2 up: ", sm[4])
	for w in wall_samples:
		print("  edge ", w[0], " (", w[2], " high) dry: ", gen.column_info(w[0].x, w[0].z))
		print("      wet ", w[1], ": ", gen.column_info(w[1].x, w[1].z))
	for h in holes.slice(0, 8):
		print("  hole ", h[0], " column y-3..y+1: ", h[1])
	quit(0)
