class_name FarLayer
extends Node3D

## The far layer (ADR 0074): the world past the box of full-detail voxels, drawn the way Lay of the
## Land draws distance (docs/research/2026-10-26-lay-of-the-land-distance.md). Native FarMesh builds
## tiles of a column height-field skin on worker threads: rings of doubling cell size (25 cm at the
## box's edge, 8 m at the horizon) that morph into their parent ring in the shader (CDLOD), flat water
## sheets, trees as instances to a few hundred metres and a canopy baked into the skin beyond.
##
## The far layer is rendered by its own camera into a SubViewport (`FarView`, transparent where there
## is no far geometry, lit by its own copy of the sun, fogged by a copy of the environment without
## tonemap or glow). The sky shader and the terrain shaders' horizon blend read that texture at the
## pixel, so the near box fades per pixel into the far layer behind it (ADR 0073's blend into the sky
## became a blend into whatever is behind), never a dither, and nothing of the far layer is ever drawn
## in front of the near voxels: it only shows where the near box has nothing.

const LAYER_BIT := 19            # visual layer 20: far meshes, the far sun and the far camera only
const R0_M := 48.0               # level 0 (25 cm cells) reaches this far; level L ends at R0_M * 2^L (a cell stays
                                 # about 5 px at 1280x720; 96 m doubled every ring and quadrupled the triangles)
const MORPH_FROM := 0.62         # a tile morphs into its parent from this fraction of its ring's radius
const MAX_IN_FLIGHT := 6         # tiles queued or building at once
const APPLY_PER_FRAME := 3       # tile meshes uploaded per frame (a level 0 tile is ~8 k triangles)
const FREE_PER_FRAME := 4        # stale tiles recycled per frame
const POOL_MAX := 400            # idle nodes kept per kind; beyond that one is freed
const UPDATE_EVERY_S := 0.15
const FLOATS_PER_TREE := 7
const EDIT_SCAN_S := 0.7         # edited chunk columns are re-read this often
const EDIT_SCANS_PER_PASS := 6
const EDIT_SPAN_CHUNKS := 20     # chunks of 2 m read around the generated ground per edited column (-16 .. +24 m)

var world: World
var atmosphere: Atmosphere
var far_mesh: FarMesh
var far_m := 1536.0
var max_level := 4
var ground_mats: Array[ShaderMaterial] = []
var water_mats: Array[ShaderMaterial] = []
var tree_mats: Array[ShaderMaterial] = []
var crown_meshes: Array[Mesh] = []      # per tree kind (FarMesh.TREE_KINDS): a lumpy crown in the unit sphere
var trunk_mesh: Mesh
var leaf_colors := PackedColorArray()   # per tree kind (oak, spruce, acacia, jungle, birch), linear
var trunk_color := Color(0.33, 0.24, 0.15)
var sub: SubViewport
var far_root: Node3D       # inside the far view's own world: every far mesh and the far sun live here
var sub_cam: Camera3D
var env_bg: Environment
var far_sun: DirectionalLight3D
var tiles := {}        # key -> Tile (drawn)
var pending := {}      # key -> true while a worker builds it
var _threads: Array[Thread] = []   # the layer's own workers: the engine's WorkerThreadPool runs the
var _jobs: Array = []               # renderer's per-frame group tasks, which a 20 ms tile build would stall
var _jobs_mutex := Mutex.new()
var _jobs_sem := Semaphore.new()
var _quit := false
var _done: Array = []  # [key, level, tx, tz, Dictionary] handed back by the workers
var _done_mutex := Mutex.new()
var _since := 1.0
var overrides := {}      # Vector2i chunk column -> PackedInt32Array(256) real surface heights where edited (-1 = as generated)
var dirty := false       # overrides changed since the last save (main.gd writes them with the world)
var _dirty := {}         # Vector2i chunk column -> true: edited since the last scan
var _edit_since := 0.0
var _scans := {}         # Vector2i -> true while a worker re-reads it
var _scanned: Array = [] # [Vector2i, PackedInt32Array] from the scan workers
var _overrides_snapshot := {}   # what the build workers read (replaced whole, never written in place)
var _last_p := Vector3(1e9, 0, 0)
var _st := {"tiles": 0, "pending": 0, "built": 0, "build_ms_max": 0.0, "build_ms_sum": 0.0, "apply_ms_max": 0.0,
	"tris": 0, "trees": 0, "levels": {}, "texture": "", "edited_columns": 0, "edit_scans": 0, "edit_rebuilds": 0,
	"update_ms_max": 0.0, "free_ms_max": 0.0, "frame_ms_max": 0.0, "freed": 0}
var _to_free: Array = []   # tiles no longer wanted, recycled a few per frame
var _pool_mesh: Array[MeshInstance3D] = []        # hidden, with their ArrayMesh: reused by the next tile (a
var _pool_multi: Array[MultiMeshInstance3D] = []  # RenderingServer free blocks on the render thread, ADR 0020)


class Tile:
	extends RefCounted
	var key: String
	var level: int
	var tx: int
	var tz: int
	var size: int          # edge in voxels
	var nodes: Array[Node3D] = []
	var tris := 0
	var trees := 0

	func rect() -> Rect2:
		return Rect2(tx * size, tz * size, size, size)


func setup(p_world: World, p_atmosphere: Atmosphere) -> void:
	world = p_world
	atmosphere = p_atmosphere
	far_m = Settings.far_distance_m
	max_level = 0
	while R0_M * pow(2.0, max_level) < far_m and max_level < FarMesh.MAX_LEVEL:
		max_level += 1
	far_mesh = FarMesh.new()
	far_mesh.configure(world.generator.gen)
	var colors := FarColors.material_colors()
	far_mesh.set_material_colors(colors)
	for kind in [GameMaterials.LEAVES, GameMaterials.SPRUCE_LEAVES, GameMaterials.ACACIA_LEAVES, GameMaterials.JUNGLE_LEAVES, GameMaterials.BIRCH_LEAVES]:
		leaf_colors.append(colors[kind] if kind < colors.size() else Color(0.3, 0.5, 0.2))
	trunk_color = colors[GameMaterials.WOOD] if GameMaterials.WOOD < colors.size() else trunk_color
	_make_materials()
	for kind in FarMesh.TREE_KINDS:
		crown_meshes.append(_crown_mesh(kind))
	trunk_mesh = BoxMesh.new()
	_make_view()
	_start_threads()
	# the near box's materials and the sky blend into the far view's texture at the pixel
	var tex := sub.get_texture()
	for m in world.horizon_materials():
		m.set_shader_parameter("far_tex", tex)
		m.set_shader_parameter("far_on", true)
	atmosphere.sky_material.set_shader_parameter("far_tex", tex)
	atmosphere.sky_material.set_shader_parameter("far_on", true)
	# the main sun lights the near box only; the far layer has its own, shadowless copy
	atmosphere.sun.light_cull_mask &= ~(1 << LAYER_BIT)


## A far crown as FineGen's trees read from 50-400 m: a handful of leaf clumps (lumpy, each a little
## lighter or darker: UV.x is the shade the shader multiplies in) inside the unit sphere the native
## instance transform scales. Oaks and jungle trees are clusters of clumps, birches slender stacks,
## spruces a cone of flattened layers, acacias flat plates.
func _crown_mesh(kind: int) -> Mesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var rng := RandomNumberGenerator.new()
	rng.seed = 1000 + kind
	var shade := func() -> float: return rng.randf_range(0.78, 1.18)
	match kind:   # WorldGen::TreeKind: 0 oak, 1 spruce, 2 acacia, 3 jungle, 4 birch
		1:
			var layers := [[-0.75, 0.95, 0.4], [-0.25, 0.72, 0.38], [0.25, 0.48, 0.36], [0.7, 0.24, 0.42]]
			for l in layers:
				_add_clump(st, Vector3(rng.randf_range(-0.05, 0.05), l[0], rng.randf_range(-0.05, 0.05)), l[1], l[2], shade.call(), 7)
		2:
			_add_clump(st, Vector3(0.0, 0.2, 0.0), 0.55, 0.14, shade.call())
			for i in 3:
				var a := i * TAU / 3.0 + rng.randf_range(-0.3, 0.3)
				_add_clump(st, Vector3(cos(a) * 0.5, rng.randf_range(0.05, 0.4), sin(a) * 0.5), rng.randf_range(0.5, 0.65), 0.13, shade.call())
		4:
			for i in 5:
				var a := i * 2.4 + rng.randf_range(-0.4, 0.4)
				var y := -0.6 + i * 0.3
				_add_clump(st, Vector3(cos(a) * 0.3, y, sin(a) * 0.3), rng.randf_range(0.36, 0.46), 0.38, shade.call())
			_add_clump(st, Vector3(0.0, 0.75, 0.0), 0.3, 0.3, shade.call())
		_:   # oak, jungle
			_add_clump(st, Vector3(0.0, 0.2, 0.0), 0.58, 0.5, shade.call())
			for i in 6:
				var a := i * TAU / 6.0 + rng.randf_range(-0.3, 0.3)
				_add_clump(st, Vector3(cos(a) * 0.52, rng.randf_range(-0.2, 0.15), sin(a) * 0.52), rng.randf_range(0.4, 0.5), rng.randf_range(0.35, 0.45), shade.call())
			for i in 2:
				var a := rng.randf_range(0.0, TAU)
				_add_clump(st, Vector3(cos(a) * 0.3, -0.5, sin(a) * 0.3), 0.42, 0.35, shade.call())
	return st.commit()


## One leaf clump: a rounded bipyramid of two rings (smooth normals), radius rx, half height ry.
func _add_clump(st: SurfaceTool, c: Vector3, rx: float, ry: float, shade: float, n := 6) -> void:
	var ring := func(y: float, r: float) -> Array[Vector3]:
		var out: Array[Vector3] = []
		for i in n:
			var a := i * TAU / n
			out.append(Vector3(cos(a) * r, y, sin(a) * r))
		return out
	var rings: Array = [ring.call(0.5, 0.9), ring.call(-0.5, 0.75)]
	var top := Vector3(0.0, 1.0, 0.0)
	var bottom := Vector3(0.0, -1.0, 0.0)
	var add := func(p: Vector3) -> void:
		st.set_normal(p.normalized())
		st.set_uv(Vector2(shade, 2.0))
		st.add_vertex(c + Vector3(p.x * rx, p.y * ry, p.z * rx))
	# Godot front faces wind clockwise seen from outside
	for i in n:
		var j := (i + 1) % n
		add.call(top); add.call(rings[0][j]); add.call(rings[0][i])
		add.call(rings[0][i]); add.call(rings[0][j]); add.call(rings[1][j])
		add.call(rings[0][i]); add.call(rings[1][j]); add.call(rings[1][i])
		add.call(rings[1][i]); add.call(rings[1][j]); add.call(bottom)


func _make_materials() -> void:
	var ground_shader: Shader = load("res://assets/shaders/far_ground.gdshader")
	var water_shader: Shader = load("res://assets/shaders/far_water.gdshader")
	var tree_shader: Shader = load("res://assets/shaders/far_tree.gdshader")
	var near_cut := Settings.m2v(Settings.view_distance_m * Settings.horizon_fade_frac)
	for level in max_level + 1:
		var r_end := Settings.m2v(R0_M * pow(2.0, level))
		var r_start := r_end * MORPH_FROM
		var gm := ShaderMaterial.new()
		gm.shader = ground_shader
		gm.set_shader_parameter("morph_start", r_start)
		gm.set_shader_parameter("morph_end", r_end)
		gm.set_shader_parameter("near_cut", near_cut)
		gm.set_shader_parameter("speckle", Settings.far_speckle)
		gm.set_shader_parameter("relief", Settings.far_relief)
		ground_mats.append(gm)
		var wm := ShaderMaterial.new()
		wm.shader = water_shader
		wm.set_shader_parameter("near_cut", near_cut)
		water_mats.append(wm)
		var tm := ShaderMaterial.new()
		tm.shader = tree_shader
		# trees shrink away only where the next level bakes the canopy into the skin instead
		var last := level == FarMesh.TREE_LEVEL
		tm.set_shader_parameter("morph_start", r_start if last else 1e9)
		tm.set_shader_parameter("morph_end", r_end if last else 2e9)
		tm.set_shader_parameter("near_cut", near_cut)
		tree_mats.append(tm)


func _make_view() -> void:
	sub = SubViewport.new()
	sub.name = "FarView"
	sub.transparent_bg = true
	# its own world: the far camera then culls the far meshes only, not the thousands of voxel block
	# instances of the near box (a second cull of those every frame stalled a 20 m/s flight, P13)
	sub.own_world_3d = true
	sub.use_hdr_2d = true
	sub.render_target_update_mode = SubViewport.UPDATE_DISABLED if Settings.far_debug == "noview" else SubViewport.UPDATE_ALWAYS
	sub.positional_shadow_atlas_size = 0
	sub.msaa_3d = Viewport.MSAA_DISABLED
	sub.screen_space_aa = Viewport.SCREEN_SPACE_AA_DISABLED
	sub.use_debanding = true
	RenderingServer.viewport_set_measure_render_time(sub.get_viewport_rid(), true)
	sub.size = Vector2i(get_viewport().get_visible_rect().size * Settings.far_scale)
	add_child(sub)
	env_bg = atmosphere.env.duplicate()
	env_bg.tonemap_mode = Environment.TONE_MAPPER_LINEAR
	env_bg.tonemap_exposure = 1.0
	env_bg.glow_enabled = false
	env_bg.ssao_enabled = false
	env_bg.ssil_enabled = false
	env_bg.sdfgi_enabled = false
	env_bg.volumetric_fog_enabled = false
	env_bg.adjustment_enabled = false
	far_root = Node3D.new()
	far_root.name = "FarWorld"
	sub.add_child(far_root)
	sub_cam = Camera3D.new()
	sub_cam.name = "FarCamera"
	sub_cam.cull_mask = 1 << LAYER_BIT
	sub_cam.environment = env_bg
	sub.add_child(sub_cam)
	sub_cam.current = true
	far_sun = DirectionalLight3D.new()
	far_sun.name = "FarSun"
	far_sun.layers = 1 << LAYER_BIT
	far_sun.light_cull_mask = 1 << LAYER_BIT
	far_sun.shadow_enabled = false
	far_root.add_child(far_sun)
	_st["texture"] = str(sub.get_texture().get_format())


func _start_threads() -> void:
	# two low-priority threads: the sim's and godot_voxel's threads already fill the cores, and a far
	# worker at normal priority preempts the main thread (30-60 ms frames in a flight, P13)
	var n := clampi(OS.get_processor_count() / 6, 1, 2)
	if Settings.far_threads > 0:
		n = Settings.far_threads
	for i in n:
		var t := Thread.new()
		t.start(_worker, Thread.PRIORITY_LOW)
		_threads.append(t)


func _worker() -> void:
	while true:
		_jobs_sem.wait()
		if _quit:
			return
		_jobs_mutex.lock()
		var job: Array = _jobs.pop_front() if not _jobs.is_empty() else []
		_jobs_mutex.unlock()
		if job.is_empty():
			continue
		if job[0] == "tile":
			_build(job[1], job[2], job[3], job[4], job[5])
		else:
			_scan(job[1], job[2], job[3])


func _enqueue(job: Array) -> void:
	_jobs_mutex.lock()
	_jobs.append(job)
	_jobs_mutex.unlock()
	_jobs_sem.post()


func _exit_tree() -> void:
	_quit = true
	for t in _threads:
		_jobs_sem.post()
	for t in _threads:
		t.wait_to_finish()
	_threads.clear()
	pending.clear()
	_scans.clear()


func _process(dt: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(dt)
	DebugCapture.note_proc("far", _pt0)


func _process_timed(dt: float) -> void:
	var cam := get_viewport().get_camera_3d()
	if cam == null or sub == null:
		return
	var t0 := Time.get_ticks_usec()
	if cam.cull_mask & (1 << LAYER_BIT):
		cam.cull_mask &= ~(1 << LAYER_BIT)
	if Settings.far_debug == "halfrate" and Engine.get_process_frames() % 2 == 1:
		# a check: the far view every other frame (a 1-frame-old far image behind the box)
		sub.render_target_update_mode = SubViewport.UPDATE_DISABLED
	elif Settings.far_debug != "noview" and sub.render_target_update_mode != SubViewport.UPDATE_ALWAYS:
		sub.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	var vp_size := Vector2i(get_viewport().get_visible_rect().size * Settings.far_scale)
	if sub.size != vp_size:
		sub.size = vp_size
	sub_cam.global_transform = cam.global_transform
	sub_cam.fov = cam.fov
	sub_cam.near = maxf(cam.near, Settings.m2v(2.0))
	sub_cam.far = Settings.m2v(far_m) * 1.7
	sub_cam.keep_aspect = cam.keep_aspect
	_sync_environment()
	far_sun.global_transform = atmosphere.sun.global_transform
	far_sun.light_color = atmosphere.sun.light_color
	far_sun.light_energy = atmosphere.sun.light_energy
	far_sun.visible = atmosphere.sun.visible
	_since += dt
	var p := cam.global_position
	var frozen := Settings.far_debug == "freeze" and Time.get_ticks_msec() > 9000
	if not frozen and (_since >= UPDATE_EVERY_S or p.distance_to(_last_p) > Settings.m2v(8.0)):
		_since = 0.0
		_last_p = p
		var t1 := Time.get_ticks_usec()
		_update_tiles(p)
		_st["update_ms_max"] = maxf(_st["update_ms_max"], (Time.get_ticks_usec() - t1) / 1000.0)
	_apply_done()
	_free_some()
	_cull_inside(cam.global_position)
	_st["frame_ms_max"] = maxf(_st["frame_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)
	if Engine.get_process_frames() % 30 == 0 and sub.render_target_update_mode != SubViewport.UPDATE_DISABLED:
		# the far view's own render time (these calls sync with the render thread: sampled, not per frame)
		var rid := sub.get_viewport_rid()
		_st["view_cpu_ms_max"] = maxf(float(_st.get("view_cpu_ms_max", 0.0)), RenderingServer.viewport_get_measured_render_time_cpu(rid))
		_st["view_gpu_ms_max"] = maxf(float(_st.get("view_gpu_ms_max", 0.0)), RenderingServer.viewport_get_measured_render_time_gpu(rid))
	_edit_since += dt
	if _edit_since >= EDIT_SCAN_S and (not _dirty.is_empty() or not _scanned.is_empty()):
		_edit_since = 0.0
		_scan_edits()


## Tiles whose farthest point is nearer than the near cut draw nothing (the shaders discard every pixel):
## hide them, mostly the level 0 tiles under the player, until the camera leaves them.
func _cull_inside(p: Vector3) -> void:
	var cut := Settings.m2v(Settings.view_distance_m * Settings.horizon_fade_frac)
	for key in tiles:
		var t: Tile = tiles[key]
		if (t.level > 1 and Settings.far_debug != "nodraw") or t.nodes.is_empty():
			continue
		var mi := t.nodes[0] as MeshInstance3D
		if mi == null:
			continue
		var aabb := mi.get_aabb()
		var far_corner := Vector3(
			aabb.position.x if absf(aabb.position.x - p.x) > absf(aabb.end.x - p.x) else aabb.end.x,
			aabb.position.y if absf(aabb.position.y - p.y) > absf(aabb.end.y - p.y) else aabb.end.y,
			aabb.position.z if absf(aabb.position.z - p.z) > absf(aabb.end.z - p.z) else aabb.end.z)
		var inside := far_corner.distance_to(p) < cut or Settings.far_debug == "nodraw"
		for n in t.nodes:
			n.visible = not inside


## The far view's environment follows the world's: fog, ambient and sky change with the time of day,
## the weather and the grade (atmosphere.gd); the tonemap and the post effects stay off here. The fog
## is thinner by Settings.far_fog: the near world's 6 / km is a beige wash by 400 m, the horizon wants
## 1-1.5 km of visibility (Lay of the Land's haze); the box's horizon blend hides the difference at 40 m.
func _sync_environment() -> void:
	var e := atmosphere.env
	env_bg.sky = e.sky
	env_bg.background_mode = e.background_mode
	env_bg.background_energy_multiplier = e.background_energy_multiplier
	env_bg.ambient_light_source = e.ambient_light_source
	env_bg.ambient_light_color = e.ambient_light_color
	env_bg.ambient_light_energy = e.ambient_light_energy
	env_bg.ambient_light_sky_contribution = e.ambient_light_sky_contribution
	env_bg.reflected_light_source = Environment.REFLECTION_SOURCE_DISABLED
	env_bg.fog_enabled = e.fog_enabled
	env_bg.fog_mode = e.fog_mode
	env_bg.fog_light_color = e.fog_light_color
	env_bg.fog_light_energy = e.fog_light_energy
	env_bg.fog_density = e.fog_density * Settings.far_fog
	env_bg.fog_sun_scatter = e.fog_sun_scatter
	env_bg.fog_aerial_perspective = e.fog_aerial_perspective
	env_bg.fog_sky_affect = e.fog_sky_affect
	env_bg.fog_height = e.fog_height
	env_bg.fog_height_density = e.fog_height_density * Settings.far_fog
	env_bg.fog_depth_curve = e.fog_depth_curve
	env_bg.fog_depth_begin = e.fog_depth_begin
	env_bg.fog_depth_end = e.fog_depth_end


## Which tiles the camera wants: a quadtree from the coarsest ring down, a tile split into its four
## children while its nearest point is inside the next finer ring (so every tile of level L that
## touches a level L + 1 tile lies wholly past L's ring and is fully morphed there: no cracks).
func _update_tiles(p: Vector3) -> void:
	var desired := {}
	var r_max := Settings.m2v(far_m)
	var t_max := far_mesh.tile_voxels(max_level)
	var tx0 := int(floor((p.x - r_max) / t_max))
	var tx1 := int(floor((p.x + r_max) / t_max))
	var tz0 := int(floor((p.z - r_max) / t_max))
	var tz1 := int(floor((p.z + r_max) / t_max))
	for tz in range(tz0, tz1 + 1):
		for tx in range(tx0, tx1 + 1):
			_select(max_level, tx, tz, p, r_max, desired)
	# build the missing ones, nearest first
	var wanted: Array = []
	for key in desired:
		if not tiles.has(key) and not pending.has(key):
			wanted.append(desired[key])
	wanted.sort_custom(func(a, b): return a[3] < b[3])
	for w in wanted:
		if pending.size() >= MAX_IN_FLIGHT:
			break
		var key: String = w[4]
		pending[key] = true
		_enqueue(["tile", key, w[0], w[1], w[2], _overrides_snapshot])
	# drop tiles no longer wanted once what replaces them is drawn (no holes): the replacement is an
	# ancestor tile or the tile's children, down the quadtree
	var stale: Array = []
	for key in tiles:
		if desired.has(key):
			continue
		var t: Tile = tiles[key]
		var anc := _covered_by_ancestor(t.level, t.tx, t.tz, desired)
		if anc == 1 or (anc == -1 and _children_drawn(t.level, t.tx, t.tz, desired, p, r_max)):
			stale.append(key)
	for key in stale:
		_to_free.append(tiles[key])
		tiles.erase(key)
	_st["tiles"] = tiles.size()
	_st["pending"] = pending.size()


## The wanted ancestor of this tile, if any: it replaces the tile once drawn.
func _covered_by_ancestor(level: int, tx: int, tz: int, desired: Dictionary) -> int:
	var l := level + 1
	var x := tx >> 1
	var z := tz >> 1
	while l <= max_level:
		var key := "%d:%d:%d" % [l, x, z]
		if desired.has(key):
			return 1 if tiles.has(key) else 0
		l += 1
		x >>= 1
		z >>= 1
	return -1   # no ancestor wanted: the children decide


## True when every wanted tile under this one is drawn; ground under it that no tile wants is out of
## range and nothing to wait for.
func _children_drawn(level: int, tx: int, tz: int, desired: Dictionary, p: Vector3, r_max: float) -> bool:
	if level == 0:
		return true
	for j in 2:
		for i in 2:
			var cl := level - 1
			var cx := tx * 2 + i
			var cz := tz * 2 + j
			var key := "%d:%d:%d" % [cl, cx, cz]
			if desired.has(key):
				if not tiles.has(key):
					return false
			elif _tile_distance(cl, cx, cz, p) < r_max and not _children_drawn(cl, cx, cz, desired, p, r_max):
				return false
	return true


func _tile_distance(level: int, tx: int, tz: int, p: Vector3) -> float:
	var size := far_mesh.tile_voxels(level)
	var r := Rect2(tx * size, tz * size, size, size)
	var q := Vector2(p.x, p.z)
	return (q - Vector2(clampf(q.x, r.position.x, r.end.x), clampf(q.y, r.position.y, r.end.y))).length()


func _select(level: int, tx: int, tz: int, p: Vector3, r_max: float, desired: Dictionary) -> void:
	var d := _tile_distance(level, tx, tz, p)
	if d >= r_max:
		return
	if level > 0 and d < Settings.m2v(R0_M * pow(2.0, level - 1)):
		for j in 2:
			for i in 2:
				_select(level - 1, tx * 2 + i, tz * 2 + j, p, r_max, desired)
		return
	var key := "%d:%d:%d" % [level, tx, tz]
	desired[key] = [level, tx, tz, d, key]


## World.set_voxel / write_chunk: the column of chunks at wc changed; the far skin re-reads it soon.
func mark_chunk(wc: Vector3i) -> void:
	_dirty[Vector2i(wc.x, wc.z)] = true


## Re-read the edited chunk columns on worker threads (the real voxels against the generated ones) and
## rebuild the far tiles over the ones whose surface changed.
func _scan_edits() -> void:
	var keys: Array = _dirty.keys()
	var started := 0
	for k in keys:
		if started >= EDIT_SCANS_PER_PASS:
			break
		if _scans.has(k):
			continue
		var ck: Vector2i = k
		var ground: int = world.generator.height_at(ck.x * Settings.CHUNK + 8, ck.y * Settings.CHUNK + 8)
		var cy0: int = maxi(0, int(floor(float(ground - Settings.m2v(16.0)) / Settings.CHUNK)))
		var bytes: PackedByteArray = world.read_column(Vector3i(ck.x, cy0, ck.y), EDIT_SPAN_CHUNKS)
		_dirty.erase(k)
		if bytes.size() != Settings.CHUNK * Settings.CHUNK * Settings.CHUNK * EDIT_SPAN_CHUNKS * 2:
			continue
		_scans[k] = true
		_enqueue(["scan", ck, cy0 * Settings.CHUNK, bytes])
		started += 1
		_st["edit_scans"] += 1
	var results: Array = []
	_done_mutex.lock()
	results = _scanned
	_scanned = []
	_done_mutex.unlock()
	var touched := {}
	for r in results:
		var ck: Vector2i = r[0]
		_scans.erase(ck)
		var tops: PackedInt32Array = r[1]
		var any := false
		for v in tops:
			if v >= 0:
				any = true
				break
		var had: bool = overrides.has(ck)
		if any:
			overrides[ck] = tops
		elif had:
			overrides.erase(ck)
		if any or had:
			touched[ck] = true
	if touched.is_empty():
		return
	dirty = true
	_st["edited_columns"] = overrides.size()
	_overrides_snapshot = overrides.duplicate(false)
	# rebuild every drawn tile over an edited column (all levels: the coarse ones show it from afar)
	for key in tiles:
		if pending.has(key):
			continue
		var t: Tile = tiles[key]
		var r := t.rect()
		for ck in touched:
			if r.intersects(Rect2(ck.x * Settings.CHUNK, ck.y * Settings.CHUNK, Settings.CHUNK, Settings.CHUNK)):
				pending[key] = true
				_enqueue(["tile", key, t.level, t.tx, t.tz, _overrides_snapshot])
				_st["edit_rebuilds"] += 1
				break


func _scan(ck: Vector2i, y0: int, bytes: PackedByteArray) -> void:
	var tops: PackedInt32Array = far_mesh.column_tops(ck, y0, EDIT_SPAN_CHUNKS, bytes)
	_done_mutex.lock()
	_scanned.append([ck, tops])
	_done_mutex.unlock()


func _build(key: String, level: int, tx: int, tz: int, _ov: Dictionary) -> void:
	var d: Dictionary = far_mesh.build_nets_tile(level, tx, tz) if Settings.far_mesher == "nets" else far_mesh.build_tile(level, tx, tz, _ov)
	_done_mutex.lock()
	_done.append([key, level, tx, tz, d])
	_done_mutex.unlock()


func _apply_done() -> void:
	var batch: Array = []
	_done_mutex.lock()
	while not _done.is_empty() and batch.size() < APPLY_PER_FRAME:
		batch.append(_done.pop_front())
	_done_mutex.unlock()
	for e in batch:
		var key: String = e[0]
		pending.erase(key)
		var t0 := Time.get_ticks_usec()
		var t := _make_tile(e[1], e[2], e[3], e[4])
		t.key = key
		if tiles.has(key):
			_free_tile(tiles[key])
		tiles[key] = t
		var ms := (Time.get_ticks_usec() - t0) / 1000.0
		_st["apply_ms_max"] = maxf(_st["apply_ms_max"], ms)
		_st["built"] += 1
		var bms: float = float(e[4].get("ms", 0.0))
		_st["build_ms_max"] = maxf(_st["build_ms_max"], bms)
		_st["build_ms_sum"] += bms
	if not batch.is_empty():
		var tris := 0
		var trees := 0
		var levels := {}
		for k in tiles:
			var t: Tile = tiles[k]
			tris += t.tris
			trees += t.trees
			levels[t.level] = int(levels.get(t.level, 0)) + 1
		_st["tris"] = tris
		_st["trees"] = trees
		_st["levels"] = levels


func _make_tile(level: int, tx: int, tz: int, d: Dictionary) -> Tile:
	var t := Tile.new()
	t.level = level
	t.tx = tx
	t.tz = tz
	t.size = far_mesh.tile_voxels(level)
	if d.has("vertices"):
		var arrays: Array = []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = d["vertices"]
		arrays[Mesh.ARRAY_NORMAL] = d["normals"]
		arrays[Mesh.ARRAY_COLOR] = d["colors"]
		arrays[Mesh.ARRAY_TEX_UV] = d["uvs"]
		arrays[Mesh.ARRAY_TEX_UV2] = d["uvs2"]
		arrays[Mesh.ARRAY_CUSTOM0] = d["custom0"]
		arrays[Mesh.ARRAY_INDEX] = d["indices"]
		var mi := _instance(ground_mats[level])
		mi.mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays, [], {}, Mesh.ARRAY_CUSTOM_RGBA8_UNORM << Mesh.ARRAY_FORMAT_CUSTOM0_SHIFT)
		t.nodes.append(mi)
		t.tris += (d["indices"] as PackedInt32Array).size() / 3
	if d.has("water_vertices"):
		var arrays: Array = []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = d["water_vertices"]
		arrays[Mesh.ARRAY_NORMAL] = d["water_normals"]
		arrays[Mesh.ARRAY_COLOR] = d["water_colors"]
		arrays[Mesh.ARRAY_INDEX] = d["water_indices"]
		var mi := _instance(water_mats[level])
		mi.mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		t.nodes.append(mi)
		t.tris += (d["water_indices"] as PackedInt32Array).size() / 3
	if d.has("crowns") and Settings.far_debug != "notrees":
		var crowns: Array = d["crowns"]
		for kind in crowns.size():
			var buf: PackedFloat32Array = crowns[kind]
			if buf.size() > 0:
				t.trees += buf.size() / 16
				t.nodes.append(_multimesh(crown_meshes[kind], buf, tree_mats[level]))
		var trunks: PackedFloat32Array = d.get("trunks", PackedFloat32Array())
		if t.trees > 0 and trunks.size() > 0:
			t.nodes.append(_multimesh(trunk_mesh, trunks, tree_mats[level]))
	return t


## A MultiMesh straight from the native buffer (12 floats of transform rows + rgba per instance).
func _multimesh(mesh: Mesh, buffer: PackedFloat32Array, mat: Material) -> MultiMeshInstance3D:
	var mmi: MultiMeshInstance3D
	if _pool_multi.is_empty():
		mmi = MultiMeshInstance3D.new()
		mmi.multimesh = MultiMesh.new()
		mmi.multimesh.transform_format = MultiMesh.TRANSFORM_3D
		mmi.multimesh.use_colors = true
		mmi.layers = 1 << LAYER_BIT
		mmi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		far_root.add_child(mmi)
	else:
		mmi = _pool_multi.pop_back()
		mmi.visible = true
	var mm := mmi.multimesh
	mm.mesh = mesh
	mm.instance_count = buffer.size() / 16
	mm.buffer = buffer
	mmi.material_override = mat
	return mmi


## A mesh instance with an empty ArrayMesh to fill, from the pool when one is idle.
func _instance(mat: Material) -> MeshInstance3D:
	var mi: MeshInstance3D
	if _pool_mesh.is_empty():
		mi = MeshInstance3D.new()
		mi.mesh = ArrayMesh.new()
		mi.layers = 1 << LAYER_BIT
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		far_root.add_child(mi)
	else:
		mi = _pool_mesh.pop_back()
		mi.visible = true
	mi.material_override = mat
	return mi


## Stale tiles leave a few per frame (recycling a hundred meshes in one frame is a visible hitch).
func _free_some() -> void:
	if _to_free.is_empty():
		return
	var t0 := Time.get_ticks_usec()
	var n := 0
	while not _to_free.is_empty() and n < FREE_PER_FRAME:
		_free_tile(_to_free.pop_back())
		n += 1
		_st["freed"] += 1
	_st["free_ms_max"] = maxf(_st["free_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)


func _free_tile(t: Tile) -> void:
	for n in t.nodes:
		n.visible = false
		if n is MeshInstance3D:
			(n as MeshInstance3D).mesh.clear_surfaces()
			if _pool_mesh.size() < POOL_MAX:
				_pool_mesh.append(n)
			else:
				n.queue_free()
		elif n is MultiMeshInstance3D:
			(n as MultiMeshInstance3D).multimesh.instance_count = 0
			if _pool_multi.size() < POOL_MAX:
				_pool_multi.append(n)
			else:
				n.queue_free()
	t.nodes.clear()


## The edited columns, saved with the world (ADR 0074): "x,z" -> 256 surface heights (-1 = as generated).
func to_dict() -> Dictionary:
	var cols := {}
	for ck in overrides:
		cols["%d,%d" % [ck.x, ck.y]] = Array(overrides[ck])
	return {"version": 1, "columns": cols}


## Restore the saved edits before the first tiles build, so a reload keeps craters and placed shapes far off.
func load_dict(d: Dictionary) -> void:
	var cols: Dictionary = d.get("columns", {})
	for key in cols:
		var p: PackedStringArray = str(key).split(",")
		var arr := PackedInt32Array(cols[key])
		if p.size() != 2 or arr.size() != 256:
			continue
		overrides[Vector2i(int(p[0]), int(p[1]))] = arr
	_overrides_snapshot = overrides.duplicate(false)
	_st["edited_columns"] = overrides.size()


## Tiles still building or waiting to be uploaded (the loading screen and the settle check count them).
func pending_count() -> int:
	return pending.size() + _done.size()


## For the run JSON and bug reports.
func get_stats() -> Dictionary:
	var s := _st.duplicate(true)
	s["build_ms_avg"] = snappedf(_st["build_ms_sum"] / maxf(1.0, _st["built"]), 0.01)
	s.erase("build_ms_sum")
	s["far_m"] = far_m
	s["max_level"] = max_level
	s["pooled"] = _pool_mesh.size() + _pool_multi.size()
	return s
