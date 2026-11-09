# Renders a block dump from blocks.exe in two looks and saves PNGs.
# "$G" --path godot --resolution 960x540 --position -30000,-30000 --script res://look.gd -- in=../out/patch.quads out=../images/look_patch [alt=25 pitch=55 yaw=0 shadow=0 mark=0]
extends SceneTree

const DEADLINE_MS := 60000
const JITTER := 0.06
# base colours per material: grass, dirt, stone, sand
const COL := [Color(0.36, 0.56, 0.22), Color(0.45, 0.32, 0.20), Color(0.50, 0.50, 0.52), Color(0.80, 0.72, 0.50)]

var _args := {}
var _frame := 0
var _shot := 0
var _mesh_flat: MeshInstance3D
var _mesh_tex: MeshInstance3D
var _cam: Camera3D


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv: PackedStringArray = a.split("=", true, 1)
		_args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	var quads: Array = []
	var eye := Vector3.ZERO
	var fwd_hint := Vector3.ZERO
	var f := FileAccess.open(_args.get("in", ""), FileAccess.READ)
	if f == null:
		push_error("no input"); quit(2); return
	while not f.eof_reached():
		var p: PackedStringArray = f.get_line().split(" ", false)
		if p.size() == 4 and p[0] == "eye":
			eye = Vector3(p[1].to_float(), p[2].to_float(), p[3].to_float())
		elif p.size() == 4 and p[0] == "fwd":
			fwd_hint = Vector3(p[1].to_float(), p[2].to_float(), p[3].to_float())
		elif p.size() == 16:
			quads.append(p)
	var root3 := Node3D.new()
	root.add_child(root3)
	_mesh_flat = _build(quads, false)
	_mesh_tex = _build(quads, true)
	root3.add_child(_mesh_flat)
	root3.add_child(_mesh_tex)
	_mesh_tex.visible = false
	var up: Vector3 = eye.normalized()
	var sun := DirectionalLight3D.new()
	sun.light_energy = 1.4
	sun.shadow_enabled = _args.get("shadow", "1") == "1"
	sun.directional_shadow_max_distance = 120
	var side: Vector3 = up.cross(Vector3(0.3, 1, 0.2)).normalized()
	sun.look_at_from_position(up * 50 + side * 30, Vector3.ZERO, up)
	root3.add_child(sun)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.55, 0.72, 0.92)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.6, 0.7, 0.85)
	e.ambient_light_energy = 0.7
	e.ssao_enabled = true
	env.environment = e
	root3.add_child(env)
	_cam = Camera3D.new()
	_cam.fov = 75
	_cam.near = 0.05
	var fwd: Vector3 = side.cross(up).normalized()
	if fwd_hint != Vector3.ZERO:  # the dump asks to look along its marked row
		fwd = (fwd_hint - up * fwd_hint.dot(up)).normalized()
	fwd = fwd.rotated(up, deg_to_rad(float(_args.get("yaw", "0"))))
	var alt: float = float(_args.get("alt", "0"))
	var pitch: float = deg_to_rad(float(_args.get("pitch", "10")))
	var look: Vector3 = (fwd * cos(pitch) - up * sin(pitch)).normalized()
	_cam.look_at_from_position(up * alt, up * alt + look * 10, up)
	root3.add_child(_cam)
	_cam.current = true
	print("quads=%d" % quads.size())


func _build(quads: Array, textured: bool) -> MeshInstance3D:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	var n := 0
	for p in quads:
		var mark: int = (int(p[0]) >> 2) & 3  # 1 = seam, 2 = cube corner (blocks.cpp)
		var on_row: bool = int(p[0]) >= 16  # a straight row of blocks, stands in for a wall or conveyor: tinted in both looks
		var mat: int = int(p[0]) & 3
		var nrm := Vector3(p[1].to_float(), p[2].to_float(), p[3].to_float())
		var v: Array[Vector3] = []
		for k in 4:
			v.append(Vector3(p[4 + 3 * k].to_float(), p[5 + 3 * k].to_float(), p[6 + 3 * k].to_float()))
		# Winding: make the triangle face along nrm (Godot front faces are clockwise).
		if (v[1] - v[0]).cross(v[2] - v[0]).dot(nrm) > 0:
			v = [v[0], v[3], v[2], v[1]]
		# Textured: the atlas carries the colour and no marker, so the seam shows as it would in game.
		var c := Color.WHITE
		if not textured:
			var j: float = rng.randf_range(-JITTER, JITTER)
			c = COL[mat]
			c = Color(c.r + j, c.g + j, c.b + j)
			if mark > 0 and _args.get("mark", "1") == "1":
				c = c.lerp(Color(0.9, 0.1, 0.7) if mark > 1 else Color(0.95, 0.45, 0.1), 0.6)
		if on_row:
			c = Color(0.4, 0.6, 1.0) if textured else c.lerp(Color(0.15, 0.35, 0.95), 0.7)
		var uv := [Vector2(0, 0), Vector2(1, 0), Vector2(1, 1), Vector2(0, 1)]
		# Textured: atlas tile per material, 4 tiles across.
		for k in 4:
			var u: Vector2 = uv[k]
			if textured:
				u = Vector2((mat + u.x) / 4.0, u.y)
			st.set_color(c)
			st.set_normal(nrm)
			st.set_uv(u)
			st.add_vertex(v[k])
		for idx in [0, 1, 2, 0, 2, 3]:
			st.add_index(n + idx)
		n += 4
	var mi := MeshInstance3D.new()
	mi.mesh = st.commit()
	var m := StandardMaterial3D.new()
	m.vertex_color_use_as_albedo = true
	m.vertex_color_is_srgb = true  # without it the colours are read as linear and wash out
	m.roughness = 1.0
	if _args.get("cull", "1") == "0":
		m.cull_mode = BaseMaterial3D.CULL_DISABLED
	if textured:
		m.albedo_texture = _atlas()
		m.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST_WITH_MIPMAPS
	mi.material_override = m
	return mi


func _atlas() -> ImageTexture:
	# 4 tiles of 16x16: per-pixel jitter of the base colour, a few darker specks (dirt/stone), grass blades.
	var img := Image.create(64, 16, false, Image.FORMAT_RGB8)
	var rng := RandomNumberGenerator.new()
	rng.seed = 3
	for t in 4:
		var base: Color = COL[t]
		for y in 16:
			for x in 16:
				var j: float = rng.randf_range(-0.09, 0.09)
				var c := Color(base.r + j, base.g + j, base.b + j)
				if t == 1 and rng.randf() < 0.12: c = c.darkened(0.35)
				if t == 2 and rng.randf() < 0.18: c = c.darkened(0.25) if rng.randf() < 0.5 else c.lightened(0.15)
				if t == 0 and rng.randf() < 0.2: c = c.lightened(0.18)
				if t == 3 and rng.randf() < 0.1: c = c.darkened(0.2)
				img.set_pixel(t * 16 + x, y, c)
	img.generate_mipmaps()
	return ImageTexture.create_from_image(img)


func _process(_dt: float) -> bool:
	_frame += 1
	if Time.get_ticks_msec() > DEADLINE_MS:
		push_error("deadline"); quit(3); return true
	if _frame == 6 or _frame == 12:
		var path: String = _args.get("out", "look") + ("_flat.png" if _shot == 0 else "_tex.png")
		root.get_viewport().get_texture().get_image().save_png(path)
		print("saved %s" % path)
		_shot += 1
		_mesh_flat.visible = false
		_mesh_tex.visible = true
	if _frame > 12:
		quit(0)
	return false
