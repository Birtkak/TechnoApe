extends SceneTree

## Renders the animals and furniture of ADR 0022 without a world, for checking
## the models: a row of sheep, sheared sheep, lamb, pig and piglet, plus a bed
## and a chair, from the front-left, the side, the back-right and above.
##   godot_console.exe --path game --script res://tools/mob_sheet.gd -- out=res://out/mobs/sheet.png
## graze=1 poses the animals grazing, walk=0.5 mid-stride.

var out_path := "res://out/mobs/sheet.png"
var _frames := 0
var _shots: Array[Image] = []
var _cams: Array[Camera3D] = []
var _models: Array[MobModel] = []
var _targets := []   # [centre, distance] per model
var _graze := 0.0
var _walk := 0.0


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"out": out_path = kv[1]
			"graze": _graze = float(kv[1])
			"walk": _walk = float(kv[1])
	var root3d := Node3D.new()
	get_root().add_child(root3d)
	var lineup := [["sheep", false, 1.0], ["sheep", true, 1.0], ["sheep", false, 0.5], ["pig", false, 1.0], ["pig", false, 0.5]]
	var x := -6.0
	for e in lineup:
		var m := MobModel.create(e[0], e[1])
		root3d.add_child(m)
		m.scale = Vector3.ONE * e[2]
		m.position = Vector3(x, 0, 0)
		_models.append(m)
		_targets.append([m.position + Vector3(0, 1.2 * e[2], 0), 4.2 * maxf(e[2], 0.7)])
		x += 6.0
	for k in ["bed", "chair"]:
		var f := FurnitureEntity.new(Vector3i(int(x), 0, -1), k, 0)
		root3d.add_child(f.make_view())
		_targets.append([Vector3(int(x) + 0.5, 0.8, 0.5 if k == "bed" else -0.5), 6.5 if k == "bed" else 4.0])
		x += 6.0
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
	sun.light_energy = 1.1
	root3d.add_child(sun)
	var env := WorldEnvironment.new()
	var e2 := Environment.new()
	e2.background_mode = Environment.BG_COLOR
	e2.background_color = Color(0.5, 0.65, 0.85)
	e2.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e2.ambient_light_color = Color(0.7, 0.7, 0.75)
	e2.ambient_light_energy = 0.9
	env.environment = e2
	root3d.add_child(env)
	var ground := MeshInstance3D.new()
	var pm := PlaneMesh.new()
	pm.size = Vector2(40, 20)
	ground.mesh = pm
	var gm := StandardMaterial3D.new()
	gm.albedo_color = Color(0.35, 0.6, 0.3)
	ground.material_override = gm
	root3d.add_child(ground)
	# a 3/4 front close-up of each (the models face +Z), then the whole line from above
	for t in _targets:
		var cam := Camera3D.new()
		cam.fov = 40
		root3d.add_child(cam)
		cam.look_at_from_position(t[0] + Vector3(0.55, 0.45, 0.7).normalized() * t[1], t[0])
		_cams.append(cam)
	var top := Camera3D.new()
	top.fov = 55
	root3d.add_child(top)
	top.look_at_from_position(Vector3(x * 0.5 - 6.0, 18, 14), Vector3(x * 0.5 - 6.0, 0, 0))
	_cams.append(top)
	_cams[0].current = true
	DisplayServer.window_set_size(Vector2i(480, 360))


func _process(delta: float) -> bool:
	_frames += 1
	if _frames == 1:
		for m in _models:
			if m.scale.x < 1.0:
				m.head.scale = Vector3.ONE * 1.45   # babies' big heads (Mob._apply_size)
	for m in _models:
		m.animate(delta, 0.0 if _walk <= 0.0 else 3.0, _graze, 0.3, 0.0, 0.0)
		if _walk > 0.0:
			for i in 4:
				m.legs[i].rotation.x = (0.6 if i in [0, 3] else -0.6) * _walk
	if _frames % 4 != 0:
		return false
	var k := _frames / 4 - 1
	if k >= 1:
		_shots.append(get_root().get_viewport().get_texture().get_image())
	if k < _cams.size():
		_cams[k].current = true
		return false
	var w := _shots[0].get_width()
	var h := _shots[0].get_height()
	var cols := 4
	var rows := int(ceil(_shots.size() / float(cols)))
	var sheet := Image.create(w * cols, h * rows, false, Image.FORMAT_RGB8)
	for i in _shots.size():
		sheet.blit_rect(_shots[i], Rect2i(0, 0, w, h), Vector2i((i % cols) * w, (i / cols) * h))
	var abs_path := ProjectSettings.globalize_path(out_path)
	DirAccess.make_dir_recursive_absolute(abs_path.get_base_dir())
	sheet.save_png(abs_path)
	print(JSON.stringify({"mob_sheet": abs_path, "views": _shots.size()}))
	quit(0)
	return false
