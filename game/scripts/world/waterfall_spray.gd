class_name WaterfallSpray
extends Node3D

## Mist at the foot of every waterfall near the camera. Every `rescan_s` the
## generator lists waterfalls within `radius_m` (`WorldGen.waterfalls_near`);
## one GPUParticles3D per fall is created or freed. Pure presentation: the
## emitters read the generator, never the voxel data, and nothing is saved.

var gen: WorldGen
var radius_m := 120.0
var rescan_s := 2.0
var _emitters := {}   # Vector3i foot -> GPUParticles3D
var _accum := 10.0
var _mesh: QuadMesh
var _material: ParticleProcessMaterial
var _count := 0
var _warm: GPUParticles3D      # shader warm-up emitter, freed after WARM_FRAMES
var _warm_frames := 0
const WARM_FRAMES := 8


func _ready() -> void:
	# soft white disc for the billboard
	var size := 32
	var img := Image.create(size, size, false, Image.FORMAT_RGBA8)
	for y in size:
		for x in size:
			var d := Vector2(x + 0.5 - size / 2.0, y + 0.5 - size / 2.0).length() / (size / 2.0)
			var a := clampf(1.0 - d, 0.0, 1.0)
			img.set_pixel(x, y, Color(1, 1, 1, a * a * 0.9))
	var tex := ImageTexture.create_from_image(img)
	var sm := StandardMaterial3D.new()
	sm.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	sm.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	sm.billboard_mode = BaseMaterial3D.BILLBOARD_PARTICLES
	sm.vertex_color_use_as_albedo = true
	sm.albedo_texture = tex
	sm.albedo_color = Color(0.92, 0.96, 1.0, 0.55)
	sm.cull_mode = BaseMaterial3D.CULL_DISABLED
	_mesh = QuadMesh.new()
	_mesh.size = Vector2(1.0, 1.0)
	_mesh.material = sm
	_material = ParticleProcessMaterial.new()
	_material.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_BOX
	_material.emission_box_extents = Vector3(1.2, 0.3, 1.2)
	_material.direction = Vector3(0, 1, 0)
	_material.spread = 40.0
	_material.initial_velocity_min = 1.5
	_material.initial_velocity_max = 4.0
	_material.gravity = Vector3(0, -1.2, 0)
	_material.damping_min = 0.8
	_material.damping_max = 1.4
	_material.scale_min = 0.8
	_material.scale_max = 2.2
	var ramp := Gradient.new()
	ramp.set_color(0, Color(1, 1, 1, 0.0))
	ramp.set_color(1, Color(1, 1, 1, 0.0))
	ramp.add_point(0.15, Color(1, 1, 1, 0.8))
	ramp.add_point(0.6, Color(1, 1, 1, 0.45))
	var ramp_tex := GradientTexture1D.new()
	ramp_tex.gradient = ramp
	_material.color_ramp = ramp_tex
	# The first GPUParticles3D compiles its process and draw shaders on the main
	# thread: ~17 ms in the frame the first waterfall came into range (fast-flight
	# benchmark). One particle in front of the camera for a few frames while the
	# world is still loading pays that cost before anyone can see it.
	_warm = _make_emitter()
	_warm.amount = 1
	_warm.lifetime = 0.2
	add_child(_warm)


func _make_emitter() -> GPUParticles3D:
	var e := GPUParticles3D.new()
	e.amount = 36
	e.lifetime = 1.8
	e.explosiveness = 0.0
	e.randomness = 0.4
	e.draw_pass_1 = _mesh
	e.process_material = _material
	e.visibility_aabb = AABB(Vector3(-8, -2, -8), Vector3(16, 14, 16))
	return e


func _process(delta: float) -> void:
	if _warm:
		var c := get_viewport().get_camera_3d()
		if c:
			_warm.global_position = c.global_position - c.global_transform.basis.z * 2.0
		_warm_frames += 1
		if _warm_frames >= WARM_FRAMES:
			_warm.queue_free()
			_warm = null
	_accum += delta
	if _accum < rescan_s or gen == null:
		return
	_accum = 0.0
	var cam := get_viewport().get_camera_3d()
	if cam == null:
		return
	var p := cam.global_position
	var radius := int(Settings.m2v(radius_m))
	var keep := {}
	for d in gen.waterfalls_near(int(floor(p.x)), int(floor(p.z)), radius):
		var key := Vector3i(d["x"], d["y"], d["z"])
		keep[key] = true
		if _emitters.has(key):
			continue
		var e := _make_emitter()
		e.position = Vector3(key.x + 0.5, key.y + 0.4, key.z + 0.5)
		add_child(e)
		_emitters[key] = e
		_count += 1
	for key in _emitters.keys():
		if not keep.has(key):
			_emitters[key].queue_free()
			_emitters.erase(key)


func emitter_count() -> int:
	return _emitters.size()
