class_name MaterialWarmup
extends Node

## Compiles every terrain pipeline while the loading screen still covers the
## view (as ParticleFx._prewarm does for the particle pools). Godot compiles a
## pipeline on a material's first draw; a world with no gas or flame in sight
## first drew the liquid/gas material when a fire started, a 20-30 ms frame on
## the render thread (and 1 s once, before the shader cache held it). One mesh
## holding every model of the library, built by the terrain's own mesher (so the
## vertex format matches the terrain's), is drawn in front of the camera, casting
## shadows, for a few frames, then freed.

const FRAMES := 8
const MESHER_PADDING := 6   # VoxelMesherBlocky::PADDING of our build (voxel_patch_fluid.py, ADR 0069 A3): a smaller buffer meshes nothing

var _mesh: MeshInstance3D
var _extra: Array[MeshInstance3D] = []
var _frames := 0


## `library` and `mesher` are the terrain's (World); call once the camera exists.
func setup(mesher: VoxelMesherBlocky, camera: Camera3D) -> void:
	var lib := mesher.library as VoxelBlockyLibrary
	if lib == null or camera == null:
		queue_free()
		return
	var n := lib.models.size()
	var P := MESHER_PADDING
	var buf := VoxelBuffer.new()
	buf.create(n * 2 + 2 * P, 2 * P + 1, 2 * P + 1)   # one voxel per model with air around it, inside the mesher's padding
	buf.set_channel_depth(VoxelBuffer.CHANNEL_TYPE, VoxelBuffer.DEPTH_16_BIT)
	for id in range(1, n):
		buf.set_voxel(id, P + id * 2, P, P, VoxelBuffer.CHANNEL_TYPE)
	var mesh := mesher.build_mesh(buf, lib.get_materials())
	if mesh == null:
		queue_free()
		return
	_mesh = MeshInstance3D.new()
	_mesh.name = "WarmupMesh"
	_mesh.mesh = mesh
	_mesh.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_ON
	_mesh.extra_cull_margin = 16384.0   # never frustum-culled: every surface must draw
	camera.add_child(_mesh)
	_mesh.transform = Transform3D(Basis.from_scale(Vector3.ONE * 0.002), Vector3(-0.5, 0.0, -1.0))


## Other materials first drawn mid-game (the mining crack, the stain coats on
## the body): one tiny quad each, drawn with the terrain mesh. Call after setup.
func add_materials(materials: Array, camera: Camera3D) -> void:
	for i in materials.size():
		var q := QuadMesh.new()
		q.size = Vector2(0.002, 0.002)
		var mi := MeshInstance3D.new()
		mi.mesh = q
		mi.material_override = materials[i]
		mi.extra_cull_margin = 16384.0
		camera.add_child(mi)
		mi.position = Vector3(-0.5 + 0.004 * i, 0.004, -1.0)
		_extra.append(mi)


## Meshes with their own materials first drawn mid-game (the crafting table, ADR 0053): drawn tiny
## with the terrain mesh, so their pipelines (vertex format and material) compile now.
func add_meshes(meshes: Array, camera: Camera3D) -> void:
	for i in meshes.size():
		var mi := MeshInstance3D.new()
		mi.mesh = meshes[i]
		mi.extra_cull_margin = 16384.0
		camera.add_child(mi)
		mi.transform = Transform3D(Basis.from_scale(Vector3.ONE * 0.0005), Vector3(0.5 - 0.004 * i, 0.004, -1.0))
		_extra.append(mi)


func _process(_delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(_delta)
	DebugCapture.note_proc("warmup", _pt0)


func _process_timed(_delta: float) -> void:
	_frames += 1
	if _frames >= FRAMES:
		if _mesh:
			_mesh.queue_free()
		for mi in _extra:
			mi.queue_free()
		queue_free()
