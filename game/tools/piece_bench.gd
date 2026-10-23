extends SceneTree

## P12 bench (prototypes/p12-rigid-pieces): cut the thickest trunk near an old-growth forest
## and time what RigidPieces does with it: detection, 30 Hz physics until it settles, writing the
## voxels back. Also times the common case, a dig in the ground. One JSON line on stdout.
##   godot --headless --path game --script res://tools/piece_bench.gd [-- biome=forest x=.. z=.. seed=..]

const Settings := preload("res://scripts/settings.gd")
const CH := Vector3i(24, 24, 24)   # 48 m cube of 12.5 cm voxels


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(int(args.get("seed", "1337")), Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var at := Vector3i(int(args.get("x", "0")), 0, int(args.get("z", "0")))
	if not args.has("x"):
		at = gen.find_nearest(args.get("biome", "old_growth_forest"), 0, 0, 40000)
	var ground := gen.height_at(at.x, at.z)
	var o := Vector3i(at.x - CH.x * 8, (ground - 96) / 16 * 16, at.z - CH.z * 8)   # world voxel of local (0,0,0)
	o.x = o.x / 16 * 16
	o.z = o.z / 16 * 16
	var sim := SimWorld.new()
	sim.configure(CH, 4, 1)
	sim.set_materials(GameMaterials.defs())
	var t0 := Time.get_ticks_msec()
	for cz in CH.z:
		for cx in CH.x:
			var bytes := gen.generate_block(o + Vector3i(cx * 16, 0, cz * 16), Vector3i(16, CH.y * 16, 16), 0, 2)
			sim.set_column_bytes(Vector3i(cx, 0, cz), CH.y, bytes, 2)
	var gen_ms := Time.get_ticks_msec() - t0
	sim.clear_dirty()
	var rp := RigidPieces.new()
	rp.setup(sim, GameMaterials.defs(), GameMaterials.ATLAS_TILES)
	rp.set_origin(o)
	rp.set_params({"region_cap": int(args.get("region_cap", "96000")), "strength": float(args.get("strength", "600"))})
	var wood := {}
	var i := 0
	for d in GameMaterials.defs():
		i += 1
		if d.get("rigid", "") == "wood":
			wood[i] = true

	# the thickest trunk within 20 m of the centre: wood columns 1 m above the ground, clustered
	var cols := {}   # Vector2i(x, z) -> ground height (local)
	var c := at - o
	for z in range(c.z - 160, c.z + 160):
		for x in range(c.x - 160, c.x + 160):
			var h := gen.height_at(x + o.x, z + o.z) - o.y
			var standing := true
			for y in [8, 16, 24, 32, 40]:   # a standing trunk: wood from 1 m to 5 m up
				standing = standing and wood.has(sim.get_voxel(Vector3i(x, h + y, z)))
			if standing:
				cols[Vector2i(x, z)] = h
	var best := []
	var used := {}
	for p: Vector2i in cols:
		if used.has(p):
			continue
		var cl := [p]
		used[p] = true
		var k := 0
		while k < cl.size():
			var q: Vector2i = cl[k]
			k += 1
			for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
				var n: Vector2i = q + d
				if not used.has(n) and cols.has(n):
					used[n] = true
					cl.append(n)
		if cl.size() > best.size():
			best = cl
	best = best.map(func(p: Vector2i) -> Vector3i: return Vector3i(p.x, cols[p], p.y))
	var out := {"at": at, "ground": ground, "gen_ms": gen_ms, "trunk_columns": best.size()}
	if best.is_empty():
		print(JSON.stringify(out))
		quit(1)
		return
	# cut a 0.5 m slab through it, 1 m above the highest ground under it
	var top := -1
	var mid := Vector3.ZERO
	for p in best:
		top = maxi(top, p.y)
		mid += Vector3(p)
	mid /= best.size()
	var cut_h := int(args.get("cut_h", "16"))   # voxels above the highest ground under the trunk
	var rad := 0.0
	for p in best:
		rad = maxf(rad, Vector2(p.x - mid.x, p.z - mid.z).length())
	rad += 6.0   # the root flare and bark around the standing columns
	var cut := []
	for z in range(int(mid.z - rad), int(mid.z + rad) + 1):
		for x in range(int(mid.x - rad), int(mid.x + rad) + 1):
			if Vector2(x - mid.x, z - mid.z).length() > rad:
				continue
			for y in range(top + cut_h, top + cut_h + 4):
				var q := Vector3i(x, y, z)
				if wood.has(sim.get_voxel(q)):
					sim.set_voxel(q, 0)
					cut.append(q + o)
	var ang := deg_to_rad(float(args.get("push_deg", "0")))   # the tree falls this way (the player stands 5 m behind)
	var from := Vector3(mid.x - 40.0 * cos(ang), top + 8.0, mid.z - 40.0 * sin(ang)) + Vector3(o)
	var ids: PackedInt32Array = rp.check(cut, from)
	out["cut_voxels"] = cut.size()
	out["cut_radius"] = rad
	out["anchors_rel"] = (rp.get_stats()["last_anchors"] as Array).map(func(a: Vector3i) -> Vector3i: return a - o - Vector3i(int(mid.x), top, int(mid.z)))
	out["anchor_mats"] = (rp.get_stats()["last_anchors"] as Array).map(func(a: Vector3i) -> String: return GameMaterials.name_of(sim.get_voxel(a - o)))
	out["check"] = rp.get_stats().duplicate()
	out["crumbs"] = rp.get_crumbs().size() / 4
	if ids.is_empty():
		out["result"] = "nothing broke off"
		var prof := {}
		for an: Vector3i in rp.get_stats()["last_anchors"]:
			var l := an - o
			for d in [Vector3i(0, 1, 0), Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]:
				var col := []
				for y in range(-1, 24):
					col.append(GameMaterials.name_of(sim.get_voxel(l + d + Vector3i(0, y, 0))).left(5))
				prof[str(l + d - Vector3i(int(mid.x), top, int(mid.z)))] = " ".join(col)
		out["anchor_profiles"] = prof
		# the support path: BFS through wood from just above the cut to the first wood touching ground
		var ground_cls := {}
		var j := 0
		for d in GameMaterials.defs():
			j += 1
			if d.get("rigid", "") == "" and d.get("cell_type", "") in ["solid", "powder"] and not d.has("micro") and not d.get("replaceable", false):
				ground_cls[j] = true
		var start := []
		for q in cut:
			var u: Vector3i = q - o + Vector3i(0, 4, 0)
			if wood.has(sim.get_voxel(u)):
				start.append(u)
		var par := {}
		for u in start:
			par[u] = u
		var qi := 0
		var hit = null
		var dirs := [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 1, 0), Vector3i(0, -1, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]
		while qi < start.size() and hit == null:
			var u: Vector3i = start[qi]
			qi += 1
			for d in dirs:
				var n: Vector3i = u + d
				var m := sim.get_voxel(n)
				if ground_cls.has(m):
					hit = u
					out["support_ground"] = [str(n - Vector3i(int(mid.x), top, int(mid.z))), GameMaterials.name_of(m)]
					break
				if wood.has(m) and not par.has(n):
					par[n] = u
					start.append(n)
		var path := []
		while hit != null and par[hit] != hit:
			path.append(hit - Vector3i(int(mid.x), top, int(mid.z)))
			hit = par[hit]
		path.reverse()
		var short := []
		for k in range(0, path.size(), maxi(1, path.size() / 40)):
			short.append("%s:%s" % [str(path[k]), GameMaterials.name_of(sim.get_voxel(path[k] + Vector3i(int(mid.x), top, int(mid.z)))).left(6)])
		out["support_path_len"] = path.size()
		out["support_path"] = short
		print(JSON.stringify(out))
		quit(1)
		return
	out["piece"] = rp.get_info(ids[0])
	var mesh_t := Time.get_ticks_usec()
	var arr: Array = rp.build_mesh(ids[0])
	out["mesh_ms"] = (Time.get_ticks_usec() - mesh_t) / 1000.0
	out["mesh_quads"] = (arr[Mesh.ARRAY_VERTEX] as PackedVector3Array).size() / 4
	var track := []
	var broken := 0
	var spawned_more := -ids.size()
	var step_ms := []
	for t in 30 * 30:
		var done: PackedInt32Array = rp.step(1.0 / 30.0)
		broken += rp.take_breaks().size() / 4
		spawned_more += rp.take_spawned().size()
		step_ms.append(rp.get_stats()["last_step_ms"])
		if done.size() > 0:
			out["settled_after_s"] = (t + 1) / 30.0
			break
		if t % 6 == 0:
			var inf: Dictionary = rp.get_info(ids[0])
			track.append([snappedf(t / 30.0, 0.01), snappedf(inf["tilt_deg"], 0.1), snappedf((inf["pos"] as Vector3).y - o.y - top, 0.1), (rp.get_stats()["last_contacts"])])
	step_ms.sort()
	out["track_t_tilt_height_contacts"] = track
	out["step_ms_p50"] = step_ms[step_ms.size() / 2]
	out["step_ms_max"] = step_ms[-1]
	var st := rp.get_stats()
	out["settle_ms"] = st["last_settle_ms"]
	out["written"] = st["last_written"]
	out["lost"] = st["last_lost"]
	out["crushed"] = st["crushed"]
	out["broken_events"] = broken
	out["limb_pieces"] = spawned_more
	out["shed_leaves"] = st["shed"]
	out["hits"] = [st["hits_ground"], st["hits_wood"], st["snapped"], st["spawned"], str(st["last_wood_hit"] - o - Vector3i(int(mid.x), top, int(mid.z))), GameMaterials.name_of(sim.get_voxel(st["last_wood_hit"] - o))]
	# the common case: a 4x4x4 dig in the ground next to the stump
	var dig := []
	var g := Vector3i(int(mid.x) + 20, gen.height_at(int(mid.x) + 20 + o.x, int(mid.z) + o.z) - o.y - 4, int(mid.z))
	for dz in 4:
		for dy in 4:
			for dx in 4:
				var q := g + Vector3i(dx, dy, dz)
				sim.set_voxel(q, 0)
				dig.append(q + o)
	rp.check(dig, from)
	out["ground_dig_check_ms"] = rp.get_stats()["last_check_ms"]
	print(JSON.stringify(out))
	quit(0)
