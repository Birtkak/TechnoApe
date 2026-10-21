extends SceneTree

## P5 push-path test (v4): native SimWorld with the Noita-style material table
## ticks at a fixed rate; dirty chunks are pushed into a godot_voxel
## VoxelTerrain through VoxelTool.paste. Measures sim/push/frame times,
## captures filmstrips and bursts, runs render-vs-sim desync checks.
## Rendered:
##   godot_console.exe --path . --script res://stream_push.gd --resolution 960x540 -- scenario=mixed tick_hz=60 camera=close
## Args after "--": threads=<n> duration=<s> tick_hz=<hz> ticks_per_frame=<n>
##   scenario=sand|water|mixed|fire|lava  camera=far|close|top|side
##   capture_every=<s> burst_at=<s> burst_frames=<n> push=0|1 sim=0|1
##   push_mode=paste|setblock settle=0|1 settle_delay=<s> mesh_block=16|32

var threads := 6
var duration := 20.0
var ticks_per_frame := 1
var tick_hz := 60.0
var scenario := "mixed"
var camera := "far"
var do_sim := true
var do_push := true
var push_mode := "paste"      # paste = VoxelTool.paste (correct under load) | setblock = try_set_block_data (stale meshes under load)
var settle_repush := false    # workaround for setblock mode only
var settle_delay := 0.25
var mesh_block := 16
var capture_every := 0.5
var burst_at := 0.0
var burst_frames := 10

var _sim: SimWorld
var _terrain: VoxelTerrain
var _vt: VoxelTool
var _cam: Camera3D
var _world_chunks := Vector3i(8, 8, 8)
var _elapsed := 0.0
var _tick_accum := 0.0
var _frame_ms := PackedFloat64Array()
var _sim_ms := 0.0
var _push_ms := 0.0
var _pushed := 0
var _push_failed := 0
var _repushes := 0
var _ticks := 0
var _started := false
var _done := false
var _last_pushed_hash := {}
var _settle_due := {}
var _strip := FrameSheet.new(320, 180, 5)
var _burst := FrameSheet.new(480, 270, 5)
var _capture_accum := 0.0
var _burst_left := 0
var _burst_started := false


func _initialize() -> void:
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"threads": threads = int(kv[1])
			"duration": duration = float(kv[1])
			"ticks_per_frame": ticks_per_frame = int(kv[1])
			"tick_hz": tick_hz = float(kv[1])
			"scenario": scenario = kv[1]
			"camera": camera = kv[1]
			"sim": do_sim = kv[1] == "1"
			"push": do_push = kv[1] == "1"
			"push_mode": push_mode = kv[1]
			"settle": settle_repush = kv[1] == "1"
			"settle_delay": settle_delay = float(kv[1])
			"mesh_block": mesh_block = int(kv[1])
			"capture_every": capture_every = float(kv[1])
			"burst_at": burst_at = float(kv[1])
			"burst_frames": burst_frames = int(kv[1])

	_sim = SimWorld.new()
	_sim.configure(_world_chunks, threads, 42)
	_sim.set_materials(SimMaterials.defs())
	_sim.set_reactions(SimMaterials.reactions())
	_sim.fill_box(Vector3i(0, 0, 0), Vector3i(128, 4, 128), SimMaterials.STONE)
	match scenario:
		"sand":
			_sim.fill_box(Vector3i(32, 60, 32), Vector3i(96, 108, 96), SimMaterials.SAND)
		"water":
			_sim.fill_box(Vector3i(40, 4, 40), Vector3i(88, 52, 88), SimMaterials.WATER)
		"fire":
			_sim.fill_box(Vector3i(30, 4, 30), Vector3i(70, 44, 70), SimMaterials.WOOD)
			_sim.fill_box(Vector3i(34, 8, 34), Vector3i(66, 40, 66), SimMaterials.AIR)
			_sim.fill_box(Vector3i(80, 4, 30), Vector3i(120, 10, 70), SimMaterials.OIL)
			for i in 4:
				_sim.ignite(Vector3i(70, 4 + i, 30 + i))
				_sim.ignite(Vector3i(69, 4 + i, 31 + i))
			_sim.fill_box(Vector3i(78, 5, 48), Vector3i(80, 7, 50), SimMaterials.FIRE)
		"lava":
			_sim.fill_box(Vector3i(20, 4, 20), Vector3i(108, 24, 108), SimMaterials.WATER)
			_sim.fill_box(Vector3i(48, 70, 48), Vector3i(80, 100, 80), SimMaterials.LAVA)
			_sim.fill_box(Vector3i(90, 24, 90), Vector3i(106, 40, 106), SimMaterials.WOOD)
		_:
			_sim.fill_box(Vector3i(16, 70, 16), Vector3i(56, 110, 56), SimMaterials.SAND)
			_sim.fill_box(Vector3i(72, 40, 72), Vector3i(112, 80, 112), SimMaterials.WATER)
			_sim.fill_box(Vector3i(50, 8, 50), Vector3i(78, 20, 78), SimMaterials.STEAM)
			for i in 6:
				_sim.fill_box(Vector3i(20 + i * 16, 4, 60), Vector3i(24 + i * 16, 40, 64), SimMaterials.STONE)

	var world := Node3D.new()
	root.add_child(world)
	_cam = Camera3D.new()
	match camera:
		"close":
			_cam.position = Vector3(64, 70, 150)
			_cam.look_at_from_position(_cam.position, Vector3(64, 35, 64), Vector3.UP)
		"top":
			_cam.position = Vector3(64, 260, 64.01)
			_cam.look_at_from_position(_cam.position, Vector3(64, 0, 64), Vector3.UP)
		"side":
			_cam.position = Vector3(-120, 45, 64)
			_cam.look_at_from_position(_cam.position, Vector3(64, 30, 64), Vector3.UP)
		_:
			_cam.position = Vector3(64, 150, 230)
			_cam.look_at_from_position(_cam.position, Vector3(64, 30, 64), Vector3.UP)
	_cam.far = 1000.0
	world.add_child(_cam)
	_cam.make_current()
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-55, 35, 0)
	sun.shadow_enabled = true
	world.add_child(sun)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.55, 0.7, 0.9)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.5, 0.5, 0.55)
	env.environment = e
	world.add_child(env)

	var lib := VoxelBlockyLibrary.new()
	var models: Array = [VoxelBlockyModelEmpty.new()]
	var cols: Array = SimMaterials.colors()
	for i in range(1, cols.size()):
		var cube := VoxelBlockyModelCube.new()
		cube.color = cols[i]
		models.append(cube)
	lib.models = models
	lib.bake()
	var mesher := VoxelMesherBlocky.new()
	mesher.library = lib
	_terrain = VoxelTerrain.new()
	_terrain.mesher = mesher
	var gen := VoxelGeneratorFlat.new()
	gen.channel = VoxelBuffer.CHANNEL_TYPE
	gen.height = -1000.0
	_terrain.generator = gen
	var mat := StandardMaterial3D.new()
	mat.vertex_color_use_as_albedo = true
	_terrain.material_override = mat
	_terrain.generate_collisions = false
	_terrain.mesh_block_size = mesh_block
	_terrain.max_view_distance = 512
	_terrain.bounds = AABB(Vector3.ZERO, Vector3(_world_chunks) * 16.0)
	world.add_child(_terrain)
	var viewer := VoxelViewer.new()
	viewer.view_distance = 512
	viewer.requires_collisions = false
	_cam.add_child(viewer)


func _process(delta: float) -> bool:
	if _done:
		return true
	_elapsed += delta
	if not _started:
		if _elapsed < 1.0:
			return false
		_started = true
		_elapsed = 0.0
		_push_dirty()
		return false
	_frame_ms.push_back(delta * 1000.0)
	var run_tick := true
	if tick_hz > 0.0:
		_tick_accum += delta
		run_tick = _tick_accum >= 1.0 / tick_hz
		if run_tick:
			_tick_accum -= 1.0 / tick_hz
	var t0 := Time.get_ticks_usec()
	if do_sim and run_tick:
		_sim.tick(ticks_per_frame)
		_ticks += ticks_per_frame
	var t1 := Time.get_ticks_usec()
	if run_tick:
		if do_push:
			_push_dirty()
		else:
			_sim.clear_dirty()
	if do_push:
		_settle()
	var t2 := Time.get_ticks_usec()
	_sim_ms += (t1 - t0) / 1000.0
	_push_ms += (t2 - t1) / 1000.0
	if capture_every > 0.0:
		_capture_accum += delta
		if _capture_accum >= capture_every:
			_capture_accum -= capture_every
			_strip.add(root.get_viewport())
	if burst_at > 0.0 and not _burst_started and _elapsed >= burst_at:
		_burst_started = true
		_burst_left = burst_frames
	if _burst_left > 0:
		_burst.add(root.get_viewport())
		_burst_left -= 1
	if _elapsed >= duration:
		_finish()
		return true
	return false


func _push_chunk(c: Vector3i) -> void:
	var bytes: PackedByteArray = _sim.get_chunk_bytes(c, 2)
	var vb := VoxelBuffer.new()
	vb.create(16, 16, 16)
	vb.set_channel_from_byte_array(VoxelBuffer.CHANNEL_TYPE, bytes)
	if push_mode == "paste":
		if _vt == null:
			_vt = _terrain.get_voxel_tool()
			_vt.channel = VoxelBuffer.CHANNEL_TYPE
		_vt.paste(Vector3i(c) * 16, vb, 1 << VoxelBuffer.CHANNEL_TYPE)
		_pushed += 1
		_last_pushed_hash[c] = hash(bytes)
		return
	if _terrain.try_set_block_data(c, vb):
		_pushed += 1
		_last_pushed_hash[c] = hash(bytes)
	else:
		_push_failed += 1


func _push_dirty() -> void:
	var dirty: Array = _sim.get_dirty_chunks()
	for c in dirty:
		_push_chunk(c)
		if settle_repush:
			_settle_due[c] = _elapsed + settle_delay
	_sim.clear_dirty()


func _settle() -> void:
	if not settle_repush:
		return
	for c in _settle_due.keys():
		if _elapsed >= _settle_due[c]:
			_settle_due.erase(c)
			_push_chunk(c)
			_repushes += 1


## Debug: compare every chunk's current sim content with what was last pushed.
func _desync_report() -> Dictionary:
	var mismatches := 0
	var never_pushed := 0
	for cz in _world_chunks.z:
		for cy in _world_chunks.y:
			for cx in _world_chunks.x:
				var c := Vector3i(cx, cy, cz)
				var h := hash(_sim.get_chunk_bytes(c, 2))
				if not _last_pushed_hash.has(c):
					never_pushed += 1
				elif _last_pushed_hash[c] != h:
					mismatches += 1
	return {"mismatched_chunks": mismatches, "never_pushed": never_pushed}


func _finish() -> void:
	_done = true
	var ft := _frame_ms.duplicate()
	ft.sort()
	var n := ft.size()
	var sum := 0.0
	for v in ft:
		sum += v
	var img := root.get_viewport().get_texture().get_image()
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://out"))
	img.save_png("res://out/push_%s_%s.png" % [scenario, camera])
	if _strip.count() > 0:
		_strip.save("res://out/strip_%s_%s.png" % [scenario, camera])
	if _burst.count() > 0:
		_burst.save("res://out/burst_%s_%s.png" % [scenario, camera])
	var hist: PackedInt32Array = _sim.material_histogram()
	var mats := {}
	var defs: Array = SimMaterials.defs()
	for i in range(1, hist.size()):
		mats[defs[i - 1]["name"]] = hist[i]
	print(JSON.stringify({
		"test": "push_path", "scenario": scenario, "camera": camera, "threads": threads, "tick_hz": tick_hz,
		"push_mode": push_mode, "settle_repush": settle_repush, "mesh_block": mesh_block,
		"frames": n, "ticks": _ticks,
		"avg_frame_ms": snappedf(sum / n, 0.01),
		"p95_frame_ms": snappedf(ft[int(n * 0.95)], 0.01),
		"p99_frame_ms": snappedf(ft[int(n * 0.99)], 0.01),
		"sim_ms_per_frame": snappedf(_sim_ms / n, 0.01),
		"push_ms_per_frame": snappedf(_push_ms / n, 0.01),
		"chunks_pushed": _pushed, "chunks_pushed_per_frame": snappedf(float(_pushed) / n, 0.1),
		"repushes": _repushes, "push_failed": _push_failed,
		"materials": mats, "burning": _sim.count_burning(),
		"unsupported_sand": _sim.count_unsupported(SimMaterials.SAND),
		"active_chunks_end": _sim.get_active_chunk_count(),
		"desync": _desync_report(),
		"voxel_stats": _terrain.get_statistics(),
	}))
	quit(0)
