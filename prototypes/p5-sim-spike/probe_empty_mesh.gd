extends SceneTree

## Render probe: does godot_voxel clear a block's mesh when the block becomes
## entirely air via try_set_block_data? Pushes a 16^3 sand cube, waits, pushes
## all-air, waits, captures frames. Also tries two workarounds on two other cubes:
##   W1: push air, then poke one voxel through VoxelTool (set_voxel air)
##   W2: push a block containing a single stone voxel, then push all-air next frame
## Run WITHOUT --headless. Output: out/probe_empty_mesh.png (filmstrip of 6 frames)

var _terrain: VoxelTerrain
var _elapsed := 0.0
var _phase := 0
var _strip := FrameSheet.new(480, 270, 3)
var _cubes := [Vector3i(2, 3, 4), Vector3i(4, 3, 4), Vector3i(6, 3, 4)]  # control, W1, W2


func _initialize() -> void:
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


func _buffer(fill_value: int, single_stone: bool = false) -> VoxelBuffer:
	var vb := VoxelBuffer.new()
	vb.create(16, 16, 16)
	vb.fill(fill_value, VoxelBuffer.CHANNEL_TYPE)
	if single_stone:
		vb.set_voxel(1, 8, 8, 8, VoxelBuffer.CHANNEL_TYPE)
	return vb


func _process(delta: float) -> bool:
	_elapsed += delta
	match _phase:
		0:
			if _elapsed > 1.0:  # blocks loaded; push three sand cubes
				for c in _cubes:
					print("push sand ", c, " ok=", _terrain.try_set_block_data(c, _buffer(2)))
				_phase = 1
		1:
			if _elapsed > 2.0:
				_strip.add(root.get_viewport())  # frame 1: three cubes visible
				# control: plain all-air push
				print("push air control ok=", _terrain.try_set_block_data(_cubes[0], _buffer(0)))
				# W1: all-air push + VoxelTool poke
				print("push air W1 ok=", _terrain.try_set_block_data(_cubes[1], _buffer(0)))
				var vt := _terrain.get_voxel_tool()
				vt.channel = VoxelBuffer.CHANNEL_TYPE
				vt.set_voxel(Vector3i(_cubes[1]) * 16 + Vector3i(8, 8, 8), 0)
				# W2: single stone voxel first
				print("push single-stone W2 ok=", _terrain.try_set_block_data(_cubes[2], _buffer(0, true)))
				_phase = 2
		2:
			if _elapsed > 2.1:
				print("push air W2 ok=", _terrain.try_set_block_data(_cubes[2], _buffer(0)))
				_phase = 3
		3:
			if _elapsed > 3.0:
				_strip.add(root.get_viewport())  # frame 2: expect all gone
				_phase = 4
		4:
			if _elapsed > 4.0:
				_strip.add(root.get_viewport())  # frame 3: still?
				DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://out"))
				_strip.save("res://out/probe_empty_mesh.png")
				print(JSON.stringify({"test": "probe_empty_mesh", "stats": _terrain.get_statistics()}))
				quit(0)
				return true
	return false
