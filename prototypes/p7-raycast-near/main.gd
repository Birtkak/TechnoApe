extends Node3D

## P7: can a GPU ray-cast micro-voxel near field run inside Godot at budget,
## composited with meshed terrain? Builds an n^3 R8 volume on the RenderingDevice
## (filled by a compute shader), ray-casts it with raycast.gdshader on its
## bounding box, draws ordinary meshes through it, uploads a "dirty" block of
## z-slices every frame (what a CPU simulation would push), and prints one JSON
## line with frame-time percentiles, upload cost and video memory.
##   godot_console.exe --path prototypes/p7-raycast-near -- size=256 dirty=64 duration=8 out=res://out

var n := 256
var dirty := 64
var burst := 0        # every `burst` frames re-upload the WHOLE volume (a black hole / 1000 TNT frame)
var carve := true     # carve=0 skips the CPU-side sphere so the burst measures the upload alone
var duration := 8.0
var _burst_ms: PackedFloat32Array = []
var _burst_frame_ms: PackedFloat32Array = []
var _all_data: Array[PackedByteArray] = []
var _after_burst := false
var out_dir := "res://out"
const CELL := 0.0625  # 6.25 cm, the ADR 0006 chisel cell (1 unit = 1 m in this spike)

var _rd: RenderingDevice
var _tex_rid: RID
var _tex: Texture2DArrayRD
var _occ_rid: RID
var _occ: Texture2DArrayRD
var _occ_shader: RID
var _occ_pipeline: RID
var _occ_uset: RID
var _gen_shader: RID
var _gen_pipeline: RID
var _gen_uset: RID
var _dirty_data: Array[PackedByteArray] = []
var _cam: Camera3D
var _centre: Vector3
var _elapsed := 0.0
var _frames: PackedFloat32Array = []
var _uploads: PackedFloat32Array = []
var _shots := 0
var _done := false


func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"size": n = int(kv[1])
			"dirty": dirty = int(kv[1])
			"burst": burst = int(kv[1])
			"carve": carve = kv[1] == "1"
			"duration": duration = float(kv[1])
			"out": out_dir = kv[1]
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	_rd = RenderingServer.get_rendering_device()
	_make_volume()
	_fill_volume()
	_build_occupancy(0, n / 8)
	_make_scene()
	# CPU copy of the dirty block so the per-frame upload carries real content
	if burst > 0:
		for z in n:
			_all_data.append(_rd.texture_get_data(_tex_rid, z))
	var z0 := (n - dirty) / 2
	for z in dirty:
		var d := _rd.texture_get_data(_tex_rid, z0 + z)
		if d.size() != n * n:
			push_error("texture_get_data returned %d bytes, expected %d" % [d.size(), n * n])
			d.resize(n * n)
		_dirty_data.append(d)


func _make_volume() -> void:
	var fmt := RDTextureFormat.new()
	fmt.texture_type = RenderingDevice.TEXTURE_TYPE_2D_ARRAY
	fmt.format = RenderingDevice.DATA_FORMAT_R8_UNORM
	fmt.width = n
	fmt.height = n
	fmt.array_layers = n
	fmt.mipmaps = 1
	fmt.usage_bits = RenderingDevice.TEXTURE_USAGE_SAMPLING_BIT | RenderingDevice.TEXTURE_USAGE_STORAGE_BIT | RenderingDevice.TEXTURE_USAGE_CAN_UPDATE_BIT | RenderingDevice.TEXTURE_USAGE_CAN_COPY_FROM_BIT
	_tex_rid = _rd.texture_create(fmt, RDTextureView.new(), [])
	_tex = Texture2DArrayRD.new()
	_tex.texture_rd_rid = _tex_rid
	var ofmt := RDTextureFormat.new()
	ofmt.texture_type = RenderingDevice.TEXTURE_TYPE_2D_ARRAY
	ofmt.format = RenderingDevice.DATA_FORMAT_R8_UNORM
	ofmt.width = n / 8
	ofmt.height = n / 8
	ofmt.array_layers = n / 8
	ofmt.mipmaps = 1
	ofmt.usage_bits = RenderingDevice.TEXTURE_USAGE_SAMPLING_BIT | RenderingDevice.TEXTURE_USAGE_STORAGE_BIT
	_occ_rid = _rd.texture_create(ofmt, RDTextureView.new(), [])
	_occ = Texture2DArrayRD.new()
	_occ.texture_rd_rid = _occ_rid


func _fill_volume() -> void:
	var shader_file: RDShaderFile = load("res://gen_volume.glsl")
	_gen_shader = _rd.shader_create_from_spirv(shader_file.get_spirv())
	_gen_pipeline = _rd.compute_pipeline_create(_gen_shader)
	var u := RDUniform.new()
	u.uniform_type = RenderingDevice.UNIFORM_TYPE_IMAGE
	u.binding = 0
	u.add_id(_tex_rid)
	_gen_uset = _rd.uniform_set_create([u], _gen_shader, 0)
	var pc := PackedByteArray()
	pc.resize(16)
	pc.encode_s32(0, n)
	pc.encode_float(4, 13.37)
	var cl := _rd.compute_list_begin()
	_rd.compute_list_bind_compute_pipeline(cl, _gen_pipeline)
	_rd.compute_list_bind_uniform_set(cl, _gen_uset, 0)
	_rd.compute_list_set_push_constant(cl, pc, pc.size())
	_rd.compute_list_dispatch(cl, n / 8, n / 8, n / 8)
	_rd.compute_list_end()
	# occupancy pass resources (dispatched by _build_occupancy)
	var occ_file: RDShaderFile = load("res://occ.glsl")
	_occ_shader = _rd.shader_create_from_spirv(occ_file.get_spirv())
	_occ_pipeline = _rd.compute_pipeline_create(_occ_shader)
	var u0 := RDUniform.new()
	u0.uniform_type = RenderingDevice.UNIFORM_TYPE_IMAGE
	u0.binding = 0
	u0.add_id(_tex_rid)
	var u1 := RDUniform.new()
	u1.uniform_type = RenderingDevice.UNIFORM_TYPE_IMAGE
	u1.binding = 1
	u1.add_id(_occ_rid)
	_occ_uset = _rd.uniform_set_create([u0, u1], _occ_shader, 0)


## Recomputes brick occupancy for brick layers [z_brick, z_brick + count).
func _build_occupancy(z_brick: int, count: int) -> void:
	var pc := PackedByteArray()
	pc.resize(16)
	pc.encode_s32(0, n / 8)
	pc.encode_s32(4, z_brick)
	var cl := _rd.compute_list_begin()
	_rd.compute_list_bind_compute_pipeline(cl, _occ_pipeline)
	_rd.compute_list_bind_uniform_set(cl, _occ_uset, 0)
	_rd.compute_list_set_push_constant(cl, pc, pc.size())
	_rd.compute_list_dispatch(cl, maxi(1, n / 64), maxi(1, n / 64), maxi(1, (count + 7) / 8))
	_rd.compute_list_end()


func _make_scene() -> void:
	var side := n * CELL
	_centre = Vector3(side * 0.5, side * 0.45, side * 0.5)
	# the ray-cast volume on its bounding box
	var box := BoxMesh.new()
	box.size = Vector3(side, side, side)
	var mi := MeshInstance3D.new()
	mi.mesh = box
	mi.position = Vector3(side, side, side) * 0.5
	var mat := ShaderMaterial.new()
	mat.shader = load("res://raycast.gdshader")
	mat.set_shader_parameter("vol", _tex)
	mat.set_shader_parameter("occ", _occ)
	mat.set_shader_parameter("n", n)
	mat.set_shader_parameter("vol_origin", Vector3.ZERO)
	mat.set_shader_parameter("cell", CELL)
	mat.set_shader_parameter("sun_dir", Vector3(0.4, 0.8, 0.3))
	mat.set_shader_parameter("max_bricks", n / 8 * 3)
	mat.set_shader_parameter("palette", PackedVector3Array([
		Vector3(0, 0, 0), Vector3(0.52, 0.52, 0.54), Vector3(0.47, 0.34, 0.22), Vector3(0.3, 0.6, 0.25),
		Vector3(0.85, 0.77, 0.47), Vector3(0.2, 0.4, 0.86), Vector3(0.3, 0.85, 0.9), Vector3(1, 0, 1)]))
	mi.material_override = mat
	mi.custom_aabb = AABB(Vector3(-side, -side, -side) * 0.5, Vector3(side, side, side))
	add_child(mi)
	# ordinary meshes crossing the volume: the depth composite test
	for i in 3:
		var b := MeshInstance3D.new()
		var bm := BoxMesh.new()
		bm.size = Vector3(1.0, 6.0, 1.0)
		b.mesh = bm
		var sm := StandardMaterial3D.new()
		sm.albedo_color = Color(0.9, 0.2, 0.2) if i != 1 else Color(0.95, 0.85, 0.2)
		b.material_override = sm
		b.position = Vector3(side * (0.3 + 0.2 * i), side * 0.5, side * (0.35 + 0.15 * i))
		add_child(b)
	var ground := MeshInstance3D.new()
	var pm := PlaneMesh.new()
	pm.size = Vector2(side * 6, side * 6)
	ground.mesh = pm
	var gm := StandardMaterial3D.new()
	gm.albedo_color = Color(0.35, 0.5, 0.3)
	ground.material_override = gm
	ground.position = Vector3(side * 0.5, -0.01, side * 0.5)
	add_child(ground)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 25, 0)
	sun.shadow_enabled = true
	add_child(sun)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.55, 0.7, 0.9)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.6, 0.6, 0.65)
	env.environment = e
	add_child(env)
	_cam = Camera3D.new()
	_cam.fov = 70
	_cam.near = 0.05
	add_child(_cam)
	_cam.current = true
	_place_camera(0.0)


func _place_camera(t: float) -> void:
	var side := n * CELL
	# first half: orbit outside; second half: hover just above the surface inside the volume
	if t < duration * 0.5:
		var a := t * 0.5
		_cam.look_at_from_position(_centre + Vector3(cos(a), 0.55, sin(a)) * side * 0.95, _centre)
	else:
		var a := t * 0.4
		var p := _centre + Vector3(cos(a) * 0.25, 0.22, sin(a) * 0.25) * side
		_cam.look_at_from_position(p, p + Vector3(-sin(a), -0.35, cos(a)))


func _process(delta: float) -> void:
	if _done:
		return
	_elapsed += delta
	if _elapsed < 1.0:
		return
	_frames.append(delta * 1000.0)
	if _after_burst:
		_burst_frame_ms.append(delta * 1000.0)  # the frame that presented the burst
		_after_burst = false
	_place_camera(_elapsed)
	if burst > 0 and _frames.size() % burst == 0:
		# everything changed at once: carve a sphere out of the CPU copy and push the whole volume
		var tb := Time.get_ticks_usec()
		var f := _frames.size() / burst
		var cx := n / 2 + ((f * 37) % 41) - 20
		var cz := n / 2 + ((f * 53) % 41) - 20
		var r := n / 6
		for z in range(maxi(0, cz - r), mini(n, cz + r) if carve else 0):
			var d := _all_data[z]
			var dz := z - cz
			for y in range(maxi(0, n / 2 - r), mini(n, n / 2 + r)):
				var dy := y - n / 2
				var rr := r * r - dz * dz - dy * dy
				if rr <= 0:
					continue
				var hw := int(sqrt(rr))
				for x in range(maxi(0, cx - hw), mini(n, cx + hw)):
					d[x + n * y] = 0
		for z in n:
			_rd.texture_update(_tex_rid, z, _all_data[z])
		_build_occupancy(0, n / 8)
		_burst_ms.append((Time.get_ticks_usec() - tb) / 1000.0)
		_after_burst = true
	# a CPU "simulation" pushed its dirty block: `dirty` consecutive z slices of n*n bytes
	var t0 := Time.get_ticks_usec()
	var z0 := (n - dirty) / 2
	if dirty > 0:
		# a sand column "falls": one cell per frame moves down inside the dirty block
		var f := _frames.size()
		var x := n / 2 + (f / 97) % 9
		var y := n - 1 - (f % 97) * (n / 100)
		for z in dirty:
			var d := _dirty_data[z]
			d[x + n * clampi(y, 0, n - 1)] = 4 if (z + f) % 3 == 0 else d[x + n * clampi(y, 0, n - 1)]
			_rd.texture_update(_tex_rid, z0 + z, d)
		_build_occupancy(z0 / 8, (dirty + 7) / 8 + 1)
	_uploads.append((Time.get_ticks_usec() - t0) / 1000.0)
	if (_elapsed >= 3.0 and _shots == 0) or (_elapsed >= duration * 0.75 and _shots == 1):
		_shots += 1
		DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(out_dir))
		get_viewport().get_texture().get_image().save_png("%s/n%d_%d.png" % [out_dir, n, _shots])
	if _elapsed >= duration + 1.0:
		_finish()


func _pct(arr: PackedFloat32Array, q: float) -> float:
	var a := arr.duplicate()
	a.sort()
	return a[clampi(int(q * a.size()), 0, a.size() - 1)] if a.size() > 0 else 0.0


func _finish() -> void:
	_done = true
	var r := {
		"test": "p7_raycast_near", "n": n, "cells": n * n * n, "cell_m": CELL, "extent_m": n * CELL,
		"resolution": [get_viewport().size.x, get_viewport().size.y],
		"frames": _frames.size(),
		"frame_ms_p50": snappedf(_pct(_frames, 0.5), 0.01), "frame_ms_p95": snappedf(_pct(_frames, 0.95), 0.01), "frame_ms_p99": snappedf(_pct(_frames, 0.99), 0.01),
		"dirty_slices": dirty, "dirty_mb_per_frame": snappedf(dirty * n * n / 1048576.0, 0.01),
		"upload_ms_p50": snappedf(_pct(_uploads, 0.5), 0.01), "upload_ms_p95": snappedf(_pct(_uploads, 0.95), 0.01),
		"burst_every": burst, "burst_carve": carve, "bursts": _burst_ms.size(), "burst_mb": snappedf(n * n * n / 1048576.0, 0.1) if burst > 0 else 0.0,
		"burst_cpu_ms_p50": snappedf(_pct(_burst_ms, 0.5), 0.01), "burst_cpu_ms_max": snappedf(_pct(_burst_ms, 1.0), 0.01),
		"frame_after_burst_ms_max": snappedf(_pct(_burst_frame_ms, 1.0), 0.01),
		"video_mem_mb": snappedf(Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0, 0.1),
		"texture_mem_mb": snappedf(Performance.get_monitor(Performance.RENDER_TEXTURE_MEM_USED) / 1048576.0, 0.1),
		"gpu": RenderingServer.get_video_adapter_name(),
	}
	print(JSON.stringify(r))
	for rid in [_gen_uset, _gen_pipeline, _gen_shader, _occ_uset, _occ_pipeline, _occ_shader]:
		if rid.is_valid():
			_rd.free_rid(rid)
	_tex.texture_rd_rid = RID()
	_occ.texture_rd_rid = RID()
	_rd.free_rid(_tex_rid)
	_rd.free_rid(_occ_rid)
	get_tree().quit(0)
