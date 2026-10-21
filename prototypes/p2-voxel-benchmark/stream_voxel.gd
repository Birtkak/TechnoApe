extends SceneTree

## P2 part B, godot_voxel (native C++): same camera flight as stream_gd.gd,
## terrain streamed by VoxelTerrain (blocky, no LOD) or VoxelLodTerrain (LOD).
## Run WITHOUT --headless:
##   godot_console.exe --path . --script res://stream_voxel.gd --resolution 960x540 -- mode=terrain
## Args after "--": mode=terrain|lod  gen=native|script  radius=<chunks>
##                  speed=<blocks/s> duration=<s>  lods=<n>  lod_distance=<blocks>

var mode := "terrain"
var gen_kind := "native"
var radius := 8
var speed := 24.0
var duration := 20.0
var lods := 5
var lod_distance := 64
var mesher_kind := "blocky"   # blocky | transvoxel (lod mode only)
var stream_kind := "octree"   # octree | clipbox (lod mode only)
var cache_generated := false

var _cam: Camera3D
var _world: Node3D
var _terrain: Node
var _frame_times := PackedFloat64Array()
var _elapsed := 0.0
var _done := false
var _mesher_name := ""


func _initialize() -> void:
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"mode": mode = kv[1]
			"gen": gen_kind = kv[1]
			"radius": radius = int(kv[1])
			"speed": speed = float(kv[1])
			"duration": duration = float(kv[1])
			"lods": lods = int(kv[1])
			"lod_distance": lod_distance = int(kv[1])
			"mesher": mesher_kind = kv[1]
			"stream": stream_kind = kv[1]
			"cache": cache_generated = kv[1] == "1"

	_world = Node3D.new()
	root.add_child(_world)
	_cam = Camera3D.new()
	_cam.position = Vector3(0, 70 if mode == "terrain" else 190, 0)
	_cam.rotation_degrees = Vector3(-35, 0, 0)
	_cam.far = 4000.0
	_world.add_child(_cam)
	_cam.make_current()
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-55, 35, 0)
	sun.shadow_enabled = true
	_world.add_child(sun)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.55, 0.7, 0.9)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.5, 0.5, 0.55)
	env.environment = e
	_world.add_child(env)

	var mat := StandardMaterial3D.new()
	mat.vertex_color_use_as_albedo = true
	mat.albedo_color = Color(0.45, 0.6, 0.3)

	var noise := FastNoiseLite.new()
	noise.seed = 1337
	noise.frequency = 0.01

	var view_blocks := radius * 16

	if mode == "lod":
		var t := VoxelLodTerrain.new()
		var blocky := VoxelMesherBlocky.new()
		var use_blocky := mesher_kind == "blocky"
		var gen := VoxelGeneratorNoise2D.new()
		gen.noise = noise
		gen.height_start = 8
		gen.height_range = 120
		if use_blocky:
			blocky.library = _make_library()
			t.mesher = blocky
			gen.channel = VoxelBuffer.CHANNEL_TYPE
			_mesher_name = "VoxelMesherBlocky"
		else:
			var tv := VoxelMesherTransvoxel.new()
			t.mesher = tv
			gen.channel = VoxelBuffer.CHANNEL_SDF
			_mesher_name = "VoxelMesherTransvoxel"
		t.generator = gen
		t.lod_count = lods
		t.lod_distance = lod_distance
		t.view_distance = view_blocks * 4
		t.material = mat
		if stream_kind == "clipbox":
			t.streaming_system = 1  # VoxelLodTerrain.STREAMING_SYSTEM_CLIPBOX
		if cache_generated:
			t.cache_generated_blocks = true
		_terrain = t
	else:
		var t := VoxelTerrain.new()
		var mesher := VoxelMesherBlocky.new()
		mesher.library = _make_library()
		t.mesher = mesher
		_mesher_name = "VoxelMesherBlocky"
		if gen_kind == "script":
			var sg := ScriptGen.new()
			sg.noise2d = noise
			t.generator = sg
		else:
			var gen := VoxelGeneratorNoise2D.new()
			gen.noise = noise
			gen.channel = VoxelBuffer.CHANNEL_TYPE
			gen.height_start = 8
			gen.height_range = 48
			t.generator = gen
		t.max_view_distance = view_blocks
		t.material_override = mat
		t.generate_collisions = false
		_terrain = t
	_world.add_child(_terrain)

	var viewer := VoxelViewer.new()
	viewer.view_distance = view_blocks if mode == "terrain" else view_blocks * 4
	viewer.requires_collisions = false
	_cam.add_child(viewer)


func _make_library() -> VoxelBlockyLibrary:
	var lib := VoxelBlockyLibrary.new()
	var models: Array = [VoxelBlockyModelEmpty.new()]
	for col in [Color(0.5, 0.5, 0.52), Color(0.45, 0.3, 0.15), Color(0.25, 0.6, 0.2)]:
		var cube := VoxelBlockyModelCube.new()
		cube.color = col
		models.append(cube)
	lib.models = models
	lib.bake()
	return lib


func _process(delta: float) -> bool:
	if _done:
		return true
	_elapsed += delta
	_frame_times.push_back(delta * 1000.0)
	_cam.position.z -= speed * delta
	if _elapsed >= duration:
		_finish()
		return true
	return false


func _finish() -> void:
	_done = true
	var ft := _frame_times.duplicate()
	ft.sort()
	var sum := 0.0
	for v in ft:
		sum += v
	var n := ft.size()
	var img := root.get_viewport().get_texture().get_image()
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://out"))
	img.save_png("res://out/stream_voxel_%s_%s_%s_%s.png" % [mode, gen_kind, mesher_kind if mode == "lod" else "blocky", stream_kind if mode == "lod" else "none"])
	var stats := {}
	if _terrain.has_method("get_statistics"):
		stats = _terrain.get_statistics()
	print(JSON.stringify({
		"test": "stream_godot_voxel",
		"mode": mode, "gen": gen_kind, "mesher": _mesher_name, "stream": stream_kind if mode == "lod" else "n/a",
		"radius": radius, "speed": speed, "duration_s": duration,
		"lods": lods if mode == "lod" else 1, "lod_distance": lod_distance if mode == "lod" else 0,
		"frames": n,
		"avg_ms": snappedf(sum / n, 0.01),
		"p50_ms": snappedf(ft[n / 2], 0.01),
		"p95_ms": snappedf(ft[int(n * 0.95)], 0.01),
		"p99_ms": snappedf(ft[int(n * 0.99)], 0.01),
		"max_ms": snappedf(ft[n - 1], 0.01),
		"mem_static_mb": snappedf(OS.get_static_memory_usage() / 1048576.0, 0.1),
		"stats": stats,
	}))
	quit(0)


## GDScript generator plugged into the native terrain: the "hybrid" data point.
## Column fills only (no caves) to keep VoxelBuffer calls per block small.
class ScriptGen:
	extends VoxelGeneratorScript
	var noise2d: FastNoiseLite

	func _get_used_channels_mask() -> int:
		return 1 << VoxelBuffer.CHANNEL_TYPE

	func _generate_block(out_buffer: VoxelBuffer, origin: Vector3i, lod: int) -> void:
		if lod != 0:
			return
		var size := out_buffer.get_size()
		var ch := VoxelBuffer.CHANNEL_TYPE
		for z in size.z:
			var wz := origin.z + z
			for x in size.x:
				var wx := origin.x + x
				var h := int(noise2d.get_noise_2d(float(wx), float(wz)) * 24.0 + 32.0)
				var top := clampi(h - origin.y, 0, size.y)
				if top <= 0:
					continue
				var stone_top := clampi(h - 4 - origin.y, 0, size.y)
				var dirt_top := clampi(h - 1 - origin.y, 0, size.y)
				if stone_top > 0:
					out_buffer.fill_area(1, Vector3i(x, 0, z), Vector3i(x + 1, stone_top, z + 1), ch)
				if dirt_top > stone_top:
					out_buffer.fill_area(2, Vector3i(x, stone_top, z), Vector3i(x + 1, dirt_top, z + 1), ch)
				if top > dirt_top:
					out_buffer.fill_area(3, Vector3i(x, dirt_top, z), Vector3i(x + 1, top, z + 1), ch)
