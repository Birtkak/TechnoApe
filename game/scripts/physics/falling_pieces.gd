class_name FallingPieces
extends Node3D

## Rigid voxel pieces (P12, ADR 0037; prototypes/p12-rigid-pieces): what a dig cuts loose (a felled
## trunk with its crown, a cut-off limb) falls as one body and settles back into voxels. The native
## RigidPieces works on the bubble's SimWorld cells; this node draws each piece in flight (16^3 mesh
## blocks, terrain_piece.gdshader, uploaded a few per frame), blends its transform between simulation ticks, keeps a settled piece's mesh
## up until the terrain paste shows its voxels, and turns what pieces break (leaves, snapped twigs)
## into break particles. The native work (finding pieces, physics, settling, meshes) runs on the sim's
## driver thread just before each sim tick (RigidPieces.set_async); this node queues it and collects
## the results between ticks. On by default; rigid_pieces=0 turns it off.

const SETTLED_FRAMES := 30      # a settled piece's mesh stays this long, until the terrain paste shows its voxels
const BREAKS_PER_TICK := 8      # break particles per simulation tick (a crown crashing down breaks thousands)
const UPLOAD_BUDGET_MS := 1.5   # mesh blocks handed to the renderer per frame (a felled crown is ~100 of them)

var rp: RigidPieces
var bubble: SimBubble
var fx: ParticleFx
var material: ShaderMaterial
var _pieces := {}        # piece id -> Node3D (its transform), one MeshInstance3D child per mesh block
var _blocks := {}        # piece id -> {block index -> MeshInstance3D}
var _uploads: Array = [] # [piece id, block index, mesh arrays], oldest first
var _settled: Array = [] # [Node3D, frames left]
var _since_tick := 0.0
var _period := 1.0 / 30.0
var tick_ms_max := 0.0
var track: Array = []   # debug: [s since the first piece, id, world position of the first piece's origin, tilt deg, contacts] every 0.5 s
var _track_ticks := 0
var _parts_max := {"collect": 0.0, "uploads": 0.0, "effects": 0.0}   # main-thread cost per part, ms


func setup(p_bubble: SimBubble, atlas_material: ShaderMaterial, p_fx: ParticleFx) -> void:
	bubble = p_bubble
	fx = p_fx
	rp = RigidPieces.new()
	rp.setup(bubble.sim, GameMaterials.defs(), GameMaterials.ATLAS_TILES)
	rp.set_origin(bubble.origin * Settings.CHUNK)
	rp.set_async(true)
	material = ShaderMaterial.new()
	material.shader = load("res://assets/shaders/terrain_piece.gdshader")
	for k in ["atlas", "atlas_tiles", "water_tile", "lava_tile", "fire_tile", "glow_tile", "grass_top_tile", "grass_side_tile", "far_fade_start", "far_fade_end"]:
		material.set_shader_parameter(k, atlas_material.get_shader_parameter(k))


## After cells were emptied (world voxels): what did they cut loose? Queued: the check runs before the
## next sim tick, and a piece shows up one tick later. Pieces topple away from `from`.
func check(cells: Array, from: Vector3) -> void:
	if rp != null and not cells.is_empty():
		rp.queue_check(cells, from)


## One simulation tick; SimBubble calls it while the SimWorld is idle (just before it starts a tick):
## take what the last jobs did, then queue the next step.
func tick(dt: float) -> void:
	if rp == null:
		return
	_period = dt
	_since_tick = 0.0
	var t0 := Time.get_ticks_usec()
	rp.set_origin(bubble.origin * Settings.CHUNK)
	var r: Dictionary = rp.collect()
	for id in r["settled"]:
		var node: Node3D = _pieces.get(id)
		_pieces.erase(id)
		_blocks.erase(id)
		if node:
			_settled.append([node, SETTLED_FRAMES])
	for id in r["spawned"]:
		var node := Node3D.new()
		node.name = "Piece%d" % id
		add_child(node)
		node.transform = rp.get_transform(id, 0.0)
		_pieces[id] = node
		_blocks[id] = {}
	var meshes: Dictionary = r["meshes"]
	for id in meshes:
		var blocks: Dictionary = meshes[id]
		for b in blocks:
			_uploads.append([id, b, blocks[b]])
	var t1 := Time.get_ticks_usec()
	_effects(r["breaks"])
	_effects(r["crumbs"])
	_parts_max["collect"] = maxf(_parts_max["collect"], (t1 - t0) / 1000.0)
	_parts_max["effects"] = maxf(_parts_max["effects"], (Time.get_ticks_usec() - t1) / 1000.0)
	if not _pieces.is_empty():
		rp.queue_step(dt)
		_track_ticks += 1
		if _track_ticks % 15 == 1 and track.size() < 60:
			var id: int = _pieces.keys()[0]
			var t := rp.get_transform(id, 1.0)
			track.append([snappedf(_track_ticks * dt, 0.1), id, (t.origin / Settings.m2v(1.0)).snapped(Vector3.ONE * 0.1), snappedf(rad_to_deg(t.basis.y.angle_to(Vector3.UP)), 1.0), rp.get_contact_info(id)])
	tick_ms_max = maxf(tick_ms_max, (Time.get_ticks_usec() - t0) / 1000.0)


## Break particles for a list of (x, y, z, material), spread over the whole list.
func _effects(br: PackedInt32Array) -> void:
	if fx == null or br.is_empty():
		return
	var step := maxi(4, (br.size() / 4 / BREAKS_PER_TICK) * 4)
	for i in range(0, br.size(), step):
		fx.block_break(Vector3i(br[i], br[i + 1], br[i + 2]), br[i + 3])


## Mesh blocks to the renderer, oldest first, within UPLOAD_BUDGET_MS a frame (at least one).
func _upload() -> void:
	var t0 := Time.get_ticks_usec()
	while not _uploads.is_empty():
		var u: Array = _uploads.pop_front()
		var blocks: Dictionary = _blocks.get(u[0], {})
		if not _pieces.has(u[0]):
			continue   # settled meanwhile
		var mi: MeshInstance3D = blocks.get(u[1])
		var arr: Array = u[2]
		if (arr[Mesh.ARRAY_INDEX] as PackedInt32Array).is_empty():
			if mi:
				mi.queue_free()
				blocks.erase(u[1])
		else:
			var m := ArrayMesh.new()
			m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arr)
			m.surface_set_material(0, material)
			if mi == null:
				mi = MeshInstance3D.new()
				mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_ON
				(_pieces[u[0]] as Node3D).add_child(mi)
				mi.set_instance_shader_parameter("piece_origin", rp.get_hash_origin(u[0]))
				blocks[u[1]] = mi
			mi.mesh = m
		if Time.get_ticks_usec() - t0 >= UPLOAD_BUDGET_MS * 1000.0:
			break
	_parts_max["uploads"] = maxf(_parts_max["uploads"], (Time.get_ticks_usec() - t0) / 1000.0)


func _process(delta: float) -> void:
	_since_tick += delta
	var a := clampf(_since_tick / _period, 0.0, 1.0)
	for id in _pieces:
		(_pieces[id] as Node3D).transform = rp.get_transform(id, a)
	if not _uploads.is_empty():
		_upload()
	for i in range(_settled.size() - 1, -1, -1):
		_settled[i][1] -= 1
		if _settled[i][1] <= 0:
			(_settled[i][0] as Node3D).queue_free()
			_settled.remove_at(i)


func stats() -> Dictionary:
	if rp == null:
		return {}
	var s := rp.get_stats()
	return {"pieces": s["pieces"], "spawned": s["spawned"], "settled": s["settled"], "broken": s["crushed"], "shed": s["shed"],
		"snapped": s["snapped"], "max_check_ms": snappedf(s["max_check_ms"], 0.01), "max_step_ms": snappedf(s["max_step_ms"], 0.01),
		"max_settle_ms": snappedf(s["max_settle_ms"], 0.01), "max_job_ms": snappedf(s["max_job_ms"], 0.01), "tick_ms_max": snappedf(tick_ms_max, 0.01),
		"tick_parts_max_ms": _parts_max, "track_m": track, "made": s.get("made", []), "rest": s.get("rest", [])}


const NOT_STANDING := -1000000


## Ground height under (x, z) if wood stands there from 1 m to 5 m up (a trunk), else NOT_STANDING.
func _standing(x: int, z: int, world: World) -> int:
	var h := world.surface_height(x, z)
	for y in [8, 16, 24, 32, 40]:
		var m := bubble.get_voxel(Vector3i(x, h + y, z))
		if m <= 0 or GameMaterials.defs()[m - 1].get("rigid", "") != "wood":
			return NOT_STANDING
	return h


## Groups of columns (Vector2i keys) that touch along x or z, `step` apart.
func _clusters(cols: Dictionary, step: int) -> Array:
	var out := []
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
			for d in [Vector2i(step, 0), Vector2i(-step, 0), Vector2i(0, step), Vector2i(0, -step)]:
				if not used.has(q + d) and cols.has(q + d):
					used[q + d] = true
					cl.append(q + d)
		out.append(cl)
	return out


## Debug (/fell, fell_test=1): cut the thickest standing trunk between `min_m` and `radius_m` from
## `around`, 2 m above its ground, falling across the line of sight (turned by `dir_deg`), so a camera
## at `around` sees it from the side. Returns {centre, columns, cut} or {}.
func fell_nearest(around: Vector3, radius_m: float, dir_deg: float, world: World, min_m := 8.0) -> Dictionary:
	var tr := find_trunk(around, radius_m, world, min_m)
	if tr.is_empty():
		return {}
	var mid: Vector2 = tr["mid"]
	var top: int = tr["top"]
	var rad: float = tr["rad"] + 6.0   # the root flare and bark around the standing columns
	var cells := []
	for z in range(int(mid.y - rad), int(mid.y + rad) + 1):
		for x in range(int(mid.x - rad), int(mid.x + rad) + 1):
			if Vector2(x - mid.x, z - mid.y).length() > rad:
				continue
			for y in range(top + 16, top + 20):
				var q := Vector3i(x, y, z)
				var m := bubble.get_voxel(q)
				if m > 0 and GameMaterials.defs()[m - 1].get("rigid", "") == "wood":
					bubble.set_voxel(q, GameMaterials.AIR)
					cells.append(q)
	var sight := (mid - Vector2(around.x, around.z)).normalized()
	var fall := Vector2(-sight.y, sight.x).rotated(deg_to_rad(dir_deg))
	var from := Vector3(mid.x - 40.0 * fall.x, top + 18.0, mid.y - 40.0 * fall.y)
	check(cells, from)
	return {"centre": Vector3(mid.x, top, mid.y), "columns": tr["columns"], "cut": cells.size()}


## Debug: the thickest standing trunk between `min_m` and `radius_m` from `around`: {mid (x, z), top
## (highest ground under it), rad (voxels from mid to its outermost standing column), columns} or {}.
func find_trunk(around: Vector3, radius_m: float, world: World, min_m := 8.0) -> Dictionary:
	# every 4th column first (0.5 m; a trunk worth felling is wider), then every column of the best trunk
	var r := int(Settings.m2v(radius_m))
	var c := Vector3i(around.floor())
	var coarse := {}
	for z in range(c.z - r, c.z + r + 1, 4):
		for x in range(c.x - r, c.x + r + 1, 4):
			var h := _standing(x, z, world)
			if h != NOT_STANDING:
				coarse[Vector2i(x, z)] = h
	var near2 := Settings.m2v(min_m) * Settings.m2v(min_m)
	var pick := []
	for cl in _clusters(coarse, 4):
		var cm := Vector2.ZERO
		for q: Vector2i in cl:
			cm += Vector2(q)
		cm /= cl.size()
		if cl.size() > pick.size() and (cm - Vector2(around.x, around.z)).length_squared() >= near2:
			pick = cl
	var cols := {}
	for p: Vector2i in pick:
		for dz in range(-3, 4):
			for dx in range(-3, 4):
				var q := p + Vector2i(dx, dz)
				if not cols.has(q):
					var h := _standing(q.x, q.y, world)
					if h != NOT_STANDING:
						cols[q] = h
	var best := []
	for cl in _clusters(cols, 1):
		if cl.size() > best.size():
			best = cl
	if best.is_empty():
		return {}
	var mid := Vector2.ZERO
	var top := -1000000
	for p: Vector2i in best:
		mid += Vector2(p)
		var gh: int = cols[p]
		top = maxi(top, gh)
	mid /= best.size()
	var rad := 0.0
	for p: Vector2i in best:
		rad = maxf(rad, (Vector2(p) - mid).length())
	return {"mid": mid, "top": top, "rad": rad, "columns": best.size()}


## Debug (/chop): where a player with the biggest cube brush (1 m) digs to chop through a trunk 1.5 m
## above its ground: the snapped brush cubes over its cross-section (bark included), nearest to `from`
## first. Returns {centre, hits: [Vector3i aimed voxel per dig], size}.
func chop_plan(tr: Dictionary, from: Vector3) -> Dictionary:
	var size := 8
	var mid: Vector2 = tr["mid"]
	var top: int = tr["top"]
	var rad: float = tr["rad"] + 2.0
	var y := int(floor(float(top + Settings.m2v(1.5)) / size)) * size
	var hits := []
	var seen := {}
	for z in range(int(mid.y - rad), int(mid.y + rad) + 1):
		for x in range(int(mid.x - rad), int(mid.x + rad) + 1):
			if Vector2(x - mid.x, z - mid.y).length() > rad:
				continue
			var cell := Vector2i(floori(float(x) / size) * size, floori(float(z) / size) * size)
			if not seen.has(cell):
				seen[cell] = true
				hits.append(Vector3i(cell.x + size / 2, y + size / 2, cell.y + size / 2))
	hits.sort_custom(func(a: Vector3i, b: Vector3i) -> bool:
		return Vector2(a.x - from.x, a.z - from.z).length_squared() < Vector2(b.x - from.x, b.z - from.z).length_squared())
	return {"centre": Vector3(mid.x, top, mid.y), "hits": hits, "size": size}


## Debug (/fell pillar): an oak log pillar (1 m square, 8 m tall, a leaf crown) on the ground 12 m
## ahead of `eye`, cut at 1 m so it falls across the view. A clean case to film; returns like fell_nearest.
func fell_pillar(eye: Vector3, forward: Vector3, world: World) -> Dictionary:
	var f := Vector2(forward.x, forward.z).normalized()
	var at := Vector2(eye.x, eye.z) + f * Settings.m2v(12.0)
	var g := world.surface_height(int(at.x), int(at.y))
	var lo := Vector3i(int(at.x) - 4, g, int(at.y) - 4)
	for y in 64:
		for z in 8:
			for x in 8:
				bubble.set_voxel(lo + Vector3i(x, y, z), GameMaterials.OAK_BARK if x == 0 or z == 0 or x == 7 or z == 7 else GameMaterials.WOOD)
	for y in range(56, 88):
		for z in range(-12, 20):
			for x in range(-12, 20):
				var d := Vector3(x - 3.5, (y - 70.0) * 1.3, z - 3.5)
				var q := lo + Vector3i(x, y, z)
				if d.length() < 13.0 and bubble.get_voxel(q) == GameMaterials.AIR:
					bubble.set_voxel(q, GameMaterials.LEAVES)
	var cells := []
	for y in range(8, 12):
		for z in 8:
			for x in 8:
				var q := lo + Vector3i(x, y, z)
				bubble.set_voxel(q, GameMaterials.AIR)
				cells.append(q)
	var fall := Vector2(-f.y, f.x)
	var from := Vector3(at.x - 40.0 * fall.x, g + 10.0, at.y - 40.0 * fall.y)
	check(cells, from)
	return {"centre": Vector3(at.x, g, at.y), "columns": 64, "cut": cells.size()}
