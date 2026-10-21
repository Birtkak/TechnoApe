extends SceneTree

## Render probe: do rapid successive try_set_block_data calls on the same block
## lose the final state? Three cubes:
##   A: alternate sand/air every frame for 90 frames, ending in air
##   B: lower the sand level one layer per frame (16 -> 0), ending in air
##   C: same as B, then re-push the final (air) state 0.5 s later
## Expect all three invisible at the end if updates are not lost.
## Output: out/probe_rapid.png (3 frames: before, right after, +1.5 s)

var _terrain: VoxelTerrain
var _elapsed := 0.0
var _phase := 0
var _frame := 0
var _strip := FrameSheet.new(480, 270, 3)
var _cubes := [Vector3i(2, 3, 4), Vector3i(4, 3, 4), Vector3i(6, 3, 4)]
var _repushed := false


func _initialize() -> void:
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_ENABLED)  # ~60 fps like a real game
	var world := Node3D.new()
	root.add_child(world)
	var cam := Camera3D.new()
	cam.position = Vector3(72, 90, 150)
	cam.look_at_from_position(cam.position, Vector3(72, 56, 72), Vector3.UP)
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
	for col in [Color(0.5, 0.5, 0.52), Color(0.85, 0.75, 0.4)]:
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


func _level_buffer(level: int) -> VoxelBuffer:
	var vb := VoxelBuffer.new()
	vb.create(16, 16, 16)
	if level > 0:
		vb.fill_area(2, Vector3i(0, 0, 0), Vector3i(16, level, 16), VoxelBuffer.CHANNEL_TYPE)
	return vb


func _process(delta: float) -> bool:
	_elapsed += delta
	match _phase:
		0:
			if _elapsed > 1.0:
				for c in _cubes:
					_terrain.try_set_block_data(c, _level_buffer(16))
				_phase = 1
		1:
			if _elapsed > 2.0:
				_strip.add(root.get_viewport())
				_phase = 2
				_frame = 0
		2:
			# 90 frames of rapid updates
			var a_level := 16 if (_frame % 2 == 0) else 0
			if _frame == 89:
				a_level = 0
			_terrain.try_set_block_data(_cubes[0], _level_buffer(a_level))
			var b_level := maxi(16 - _frame, 0)
			_terrain.try_set_block_data(_cubes[1], _level_buffer(b_level))
			_terrain.try_set_block_data(_cubes[2], _level_buffer(b_level))
			_frame += 1
			if _frame >= 90:
				_phase = 3
				_elapsed = 0.0
		3:
			if _elapsed > 0.5 and not _repushed:
				_repushed = true
				_terrain.try_set_block_data(_cubes[2], _level_buffer(0))
				_strip.add(root.get_viewport())
			if _elapsed > 2.0:
				_strip.add(root.get_viewport())
				DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://out"))
				_strip.save("res://out/probe_rapid.png")
				print(JSON.stringify({"test": "probe_rapid", "stats": _terrain.get_statistics()}))
				quit(0)
				return true
	return false
