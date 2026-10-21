extends SceneTree

## Render probe under load: update N blocks every frame for 90 frames, lowering
## their sand level from 16 to 0 (final state = air). Half of the blocks get an
## extra "settle re-push" of the final state 0.5 s after the burst (workaround).
## If stale meshes remain only in the non-re-pushed half, the renderer loses
## updates under load and the workaround is valid.
##   godot_console.exe --path . --script res://probe_heavy.gd -- blocks=200
## Output: out/probe_heavy.png (frames: before, +0.5 s, +2 s), JSON line.

var blocks := 200
var _terrain: VoxelTerrain
var _elapsed := 0.0
var _phase := 0
var _frame := 0
var _strip := FrameSheet.new(480, 270, 3)
var _targets: Array[Vector3i] = []
var _repushed := false


func _initialize() -> void:
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_ENABLED)
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "blocks":
			blocks = int(kv[1])
	# Targets: a slab of chunks at y=3..4 across the world (up to 8x2x8 = 128) then y=5 (more).
	for cy in [3, 4, 5, 6]:
		for cz in 8:
			for cx in 8:
				if _targets.size() < blocks:
					_targets.append(Vector3i(cx, cy, cz))
	var world := Node3D.new()
	root.add_child(world)
	var cam := Camera3D.new()
	cam.position = Vector3(-120, 90, 64)
	cam.look_at_from_position(cam.position, Vector3(64, 60, 64), Vector3.UP)
	world.add_child(cam)
	cam.make_current()
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-55, 35, 0)
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
	for col in [Color(0.5, 0.5, 0.52), Color(0.85, 0.75, 0.4), Color(0.9, 0.3, 0.3)]:
		var cube := VoxelBlockyModelCube.new()
		cube.color = col
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
	_terrain.max_view_distance = 512
	_terrain.bounds = AABB(Vector3.ZERO, Vector3(128, 128, 128))
	world.add_child(_terrain)
	var viewer := VoxelViewer.new()
	viewer.view_distance = 512
	viewer.requires_collisions = false
	cam.add_child(viewer)


func _level_buffer(level: int, mat: int) -> VoxelBuffer:
	var vb := VoxelBuffer.new()
	vb.create(16, 16, 16)
	if level > 0:
		vb.fill_area(mat, Vector3i(0, 0, 0), Vector3i(16, level, 16), VoxelBuffer.CHANNEL_TYPE)
	return vb


func _process(delta: float) -> bool:
	_elapsed += delta
	match _phase:
		0:
			if _elapsed > 1.0:
				for i in _targets.size():
					# left half (x<4) sand = will be re-pushed; right half red = control
					var m := 2 if _targets[i].x < 4 else 3
					_terrain.try_set_block_data(_targets[i], _level_buffer(16, m))
				_phase = 1
		1:
			if _elapsed > 2.5:
				_strip.add(root.get_viewport())
				_phase = 2
				_frame = 0
		2:
			var level := maxi(16 - _frame, 0)
			for i in _targets.size():
				var m := 2 if _targets[i].x < 4 else 3
				_terrain.try_set_block_data(_targets[i], _level_buffer(level, m))
			_frame += 1
			if _frame >= 90:
				_phase = 3
				_elapsed = 0.0
		3:
			if _elapsed > 0.5 and not _repushed:
				_repushed = true
				_strip.add(root.get_viewport())
				for i in _targets.size():
					if _targets[i].x < 4:
						_terrain.try_set_block_data(_targets[i], _level_buffer(0, 2))
			if _elapsed > 2.5:
				_strip.add(root.get_viewport())
				DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://out"))
				_strip.save("res://out/probe_heavy.png")
				print(JSON.stringify({"test": "probe_heavy", "blocks": _targets.size(), "stats": _terrain.get_statistics()}))
				quit(0)
				return true
	return false
