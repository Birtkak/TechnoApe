class_name FallingPieces
extends Node3D

## Rigid voxel pieces (P12, ADR 0037; prototypes/p12-rigid-pieces): what a dig cuts loose (a felled
## trunk with its crown, a cut-off limb) falls as one body and settles back into voxels. The native
## RigidPieces works on the bubble's SimWorld cells; this node draws each piece in flight (one mesh,
## terrain_piece.gdshader), blends its transform between simulation ticks, keeps a settled piece's mesh
## up until the terrain paste shows its voxels, and turns what pieces break (leaves, snapped twigs)
## into break particles. Behind rigid_pieces=1 while it is a prototype.

const SETTLED_FRAMES := 30      # a settled piece's mesh stays this long, until the terrain paste shows its voxels
const BREAKS_PER_TICK := 24     # break particles per simulation tick (a crown crashing down breaks thousands)

var rp: RigidPieces
var bubble: SimBubble
var fx: ParticleFx
var material: ShaderMaterial
var _meshes := {}        # piece id -> MeshInstance3D
var _settled: Array = [] # [MeshInstance3D, frames left]
var _since_tick := 0.0
var _period := 1.0 / 30.0
var tick_ms_max := 0.0


func setup(p_bubble: SimBubble, atlas_material: ShaderMaterial, p_fx: ParticleFx) -> void:
	bubble = p_bubble
	fx = p_fx
	rp = RigidPieces.new()
	rp.setup(bubble.sim, GameMaterials.defs(), GameMaterials.ATLAS_TILES)
	rp.set_origin(bubble.origin * Settings.CHUNK)
	material = ShaderMaterial.new()
	material.shader = load("res://assets/shaders/terrain_piece.gdshader")
	for k in ["atlas", "atlas_tiles", "water_tile", "lava_tile", "fire_tile", "glow_tile", "grass_top_tile", "grass_side_tile", "far_fade_start", "far_fade_end"]:
		material.set_shader_parameter(k, atlas_material.get_shader_parameter(k))


## After cells were emptied (world voxels): what did they cut loose? Pieces topple away from `from`.
func check(cells: Array, from: Vector3) -> int:
	if rp == null or cells.is_empty():
		return 0
	rp.set_origin(bubble.origin * Settings.CHUNK)
	var n := rp.check(cells, from).size()
	_spawn_new()
	var crumbs := rp.get_crumbs()
	for i in range(0, mini(crumbs.size(), 4 * BREAKS_PER_TICK), 4):
		if fx:
			fx.block_break(Vector3i(crumbs[i], crumbs[i + 1], crumbs[i + 2]), crumbs[i + 3])
	return n


## One simulation tick; SimBubble calls it while the SimWorld is idle (just before it starts a tick).
func tick(dt: float) -> void:
	if rp == null:
		return
	_period = dt
	_since_tick = 0.0
	if _meshes.is_empty():
		return
	var t0 := Time.get_ticks_usec()
	rp.set_origin(bubble.origin * Settings.CHUNK)
	for id in rp.step(dt):
		var mi: MeshInstance3D = _meshes.get(id)
		_meshes.erase(id)
		if mi:
			_settled.append([mi, SETTLED_FRAMES])
	_spawn_new()
	for id in rp.take_dirty_meshes():
		if _meshes.has(id):
			(_meshes[id] as MeshInstance3D).mesh = _build(id)
	var br := rp.take_breaks()
	if fx:
		var step := maxi(4, (br.size() / 4 / BREAKS_PER_TICK) * 4)   # spread over the whole list, not its start
		for i in range(0, br.size(), step):
			fx.block_break(Vector3i(br[i], br[i + 1], br[i + 2]), br[i + 3])
	tick_ms_max = maxf(tick_ms_max, (Time.get_ticks_usec() - t0) / 1000.0)


func _spawn_new() -> void:
	for id in rp.take_spawned():
		var mi := MeshInstance3D.new()
		mi.name = "Piece%d" % id
		mi.mesh = _build(id)
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_ON
		add_child(mi)
		mi.set_instance_shader_parameter("piece_origin", rp.get_hash_origin(id))
		mi.transform = rp.get_transform(id, 0.0)
		_meshes[id] = mi


func _build(id: int) -> ArrayMesh:
	var m := ArrayMesh.new()
	var arr: Array = rp.build_mesh(id)
	if not arr.is_empty() and (arr[Mesh.ARRAY_INDEX] as PackedInt32Array).size() > 0:
		m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arr)
		m.surface_set_material(0, material)
	return m


func _process(delta: float) -> void:
	_since_tick += delta
	var a := clampf(_since_tick / _period, 0.0, 1.0)
	for id in _meshes:
		(_meshes[id] as MeshInstance3D).transform = rp.get_transform(id, a)
	for i in range(_settled.size() - 1, -1, -1):
		_settled[i][1] -= 1
		if _settled[i][1] <= 0:
			(_settled[i][0] as MeshInstance3D).queue_free()
			_settled.remove_at(i)


func stats() -> Dictionary:
	if rp == null:
		return {}
	var s := rp.get_stats()
	return {"pieces": s["pieces"], "spawned": s["spawned"], "settled": s["settled"], "broken": s["crushed"], "shed": s["shed"],
		"snapped": s["snapped"], "max_check_ms": snappedf(s["max_check_ms"], 0.01), "max_step_ms": snappedf(s["max_step_ms"], 0.01),
		"max_settle_ms": snappedf(s["max_settle_ms"], 0.01), "tick_ms_max": snappedf(tick_ms_max, 0.01)}


## Debug (/fell, fell_test=1): cut the thickest standing trunk between `min_m` and `radius_m` from
## `around`, 2 m above its ground, falling across the line of sight (turned by `dir_deg`), so a camera
## at `around` sees it from the side. Returns {centre, columns, cut, pieces} or {}.
func fell_nearest(around: Vector3, radius_m: float, dir_deg: float, world: World, min_m := 8.0) -> Dictionary:
	var r := int(Settings.m2v(radius_m))
	var c := Vector3i(around.floor())
	var cols := {}
	for z in range(c.z - r, c.z + r + 1):
		for x in range(c.x - r, c.x + r + 1):
			var h := world.surface_height(x, z)
			var standing := true
			for y in [8, 16, 24, 32, 40]:   # wood from 1 m to 5 m up
				var m := bubble.get_voxel(Vector3i(x, h + y, z))
				standing = standing and m > 0 and GameMaterials.defs()[m - 1].get("rigid", "") == "wood"
				if not standing:
					break
			if standing:
				cols[Vector2i(x, z)] = h
	var near2 := Settings.m2v(min_m) * Settings.m2v(min_m)
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
				if not used.has(q + d) and cols.has(q + d):
					used[q + d] = true
					cl.append(q + d)
		var cm := Vector2.ZERO
		for q: Vector2i in cl:
			cm += Vector2(q)
		cm /= cl.size()
		if cl.size() > best.size() and (cm - Vector2(around.x, around.z)).length_squared() >= near2:
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
	var rad := 6.0
	for p: Vector2i in best:
		rad = maxf(rad, (Vector2(p) - mid).length() + 6.0)
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
	var n := check(cells, from)
	return {"centre": Vector3(mid.x, top, mid.y), "columns": best.size(), "cut": cells.size(), "pieces": n}


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
	var n := check(cells, from)
	return {"centre": Vector3(at.x, g, at.y), "columns": 64, "cut": cells.size(), "pieces": n}
