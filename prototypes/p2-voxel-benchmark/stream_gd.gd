extends SceneTree

## P2 part B, GDScript streaming: camera flies over an infinite world while a
## chunk manager generates + meshes chunks on worker threads (ChunkGd) and
## uploads a capped number per frame. Measures frame times over DURATION
## seconds, saves a screenshot, prints one JSON line. Run WITHOUT --headless:
##   godot_console.exe --path . --script res://stream_gd.gd --resolution 960x540
## Optional args after "--": radius=<chunks> speed=<blocks/s> duration=<s>

const CHUNK := ChunkGd.SIZE
var radius := 8            # horizontal radius in chunks (8 -> 17x17 columns)
var chunks_y := 4          # vertical chunks 0..3 (64 blocks)
var speed := 24.0          # camera speed, blocks/s
var duration := 20.0
var max_uploads_per_frame := 8

var _noise2d := FastNoiseLite.new()
var _noise3d := FastNoiseLite.new()
var _world: Node3D
var _cam: Camera3D
var _loaded := {}          # Vector3i -> MeshInstance3D
var _pending := {}         # Vector3i -> task id
var _results := {}         # Vector3i -> arrays (filled by worker threads)
var _results_mutex := Mutex.new()
var _frame_times := PackedFloat64Array()
var _elapsed := 0.0
var _chunks_uploaded := 0
var _done := false
var _material := StandardMaterial3D.new()


func _initialize() -> void:
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"radius": radius = int(kv[1])
			"speed": speed = float(kv[1])
			"duration": duration = float(kv[1])
	_noise2d.seed = 1337
	_noise2d.frequency = 0.01
	_noise3d.seed = 7331
	_noise3d.frequency = 0.05
	_material.vertex_color_use_as_albedo = true

	_world = Node3D.new()
	root.add_child(_world)
	_cam = Camera3D.new()
	_cam.position = Vector3(0, 70, 0)
	_cam.rotation_degrees = Vector3(-35, 0, 0)
	_cam.far = 600.0
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


func _process(delta: float) -> bool:
	if _done:
		return true
	_elapsed += delta
	_frame_times.push_back(delta * 1000.0)
	_cam.position.z -= speed * delta

	var cc := Vector3i(floori(_cam.position.x / CHUNK), 0, floori(_cam.position.z / CHUNK))
	_schedule_around(cc)
	_unload_far(cc)
	_apply_results()

	if _elapsed >= duration:
		_finish()
		return true
	return false


func _schedule_around(cc: Vector3i) -> void:
	# Nearest first so the view fills from the camera outwards.
	var wanted: Array[Vector3i] = []
	for dz in range(-radius, radius + 1):
		for dx in range(-radius, radius + 1):
			if dx * dx + dz * dz > radius * radius:
				continue
			for cy in chunks_y:
				var key := Vector3i(cc.x + dx, cy, cc.z + dz)
				if not _loaded.has(key) and not _pending.has(key):
					wanted.append(key)
	wanted.sort_custom(func(a, b):
		var da: Vector3i = (a - cc) * Vector3i(1, 0, 1)
		var db: Vector3i = (b - cc) * Vector3i(1, 0, 1)
		return da.length_squared() < db.length_squared())
	var budget := 64 - _pending.size()
	for key in wanted:
		if budget <= 0:
			break
		var id := WorkerThreadPool.add_task(_worker.bind(key), false, "chunk")
		_pending[key] = id
		budget -= 1


func _worker(key: Vector3i) -> void:
	var vox := ChunkGd.generate(key.x, key.y, key.z, _noise2d, _noise3d)
	var arrays := ChunkGd.mesh(vox)
	_results_mutex.lock()
	_results[key] = arrays
	_results_mutex.unlock()


func _apply_results() -> void:
	_results_mutex.lock()
	var ready := _results.keys()
	_results_mutex.unlock()
	var n := 0
	for key in ready:
		if n >= max_uploads_per_frame:
			break
		_results_mutex.lock()
		var arrays: Array = _results[key]
		_results.erase(key)
		_results_mutex.unlock()
		WorkerThreadPool.wait_for_task_completion(_pending[key])
		_pending.erase(key)
		var mi := MeshInstance3D.new()
		mi.mesh = ChunkGd.build_array_mesh(arrays)
		mi.material_override = _material
		mi.position = Vector3(key) * CHUNK
		_world.add_child(mi)
		_loaded[key] = mi
		_chunks_uploaded += 1
		n += 1


func _unload_far(cc: Vector3i) -> void:
	var far := (radius + 2) * (radius + 2)
	for key in _loaded.keys():
		var d: Vector3i = (key - cc) * Vector3i(1, 0, 1)
		if d.length_squared() > far:
			_loaded[key].queue_free()
			_loaded.erase(key)


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
	img.save_png("res://out/stream_gd.png")
	print(JSON.stringify({
		"test": "stream_gdscript",
		"radius": radius, "chunks_y": chunks_y, "speed": speed, "duration_s": duration,
		"frames": n,
		"avg_ms": snappedf(sum / n, 0.01),
		"p50_ms": snappedf(ft[n / 2], 0.01),
		"p95_ms": snappedf(ft[int(n * 0.95)], 0.01),
		"p99_ms": snappedf(ft[int(n * 0.99)], 0.01),
		"max_ms": snappedf(ft[n - 1], 0.01),
		"chunks_uploaded": _chunks_uploaded,
		"chunks_loaded_end": _loaded.size(),
		"pending_end": _pending.size(),
		"mem_static_mb": snappedf(OS.get_static_memory_usage() / 1048576.0, 0.1),
	}))
	for id in _pending.values():
		WorkerThreadPool.wait_for_task_completion(id)
	quit(0)
