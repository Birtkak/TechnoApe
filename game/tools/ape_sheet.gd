extends SceneTree

## Renders the player body (ApeModel) from the front, left, back and a head
## close-up into one PNG, for checking the model without a world:
##   godot_console.exe --path game --script res://tools/ape_sheet.gd -- out=res://out/ape/sheet.png

var out_path := "res://out/ape/sheet.png"
var _frames := 0
var _shots: Array[Image] = []
var _cams: Array[Camera3D] = []
var _model: ApeModel


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "out":
			out_path = kv[1]
	var root3d := Node3D.new()
	get_root().add_child(root3d)
	_model = ApeModel.new()
	root3d.add_child(_model)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-45, 35, 0)
	sun.light_energy = 1.1
	root3d.add_child(sun)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.5, 0.65, 0.85)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.7, 0.7, 0.75)
	e.ambient_light_energy = 0.9
	env.environment = e
	root3d.add_child(env)
	# model is ~3.6 voxels tall; frame it from 4 sides (front = -Z)
	var mid := Vector3(0, 1.8, 0)
	var head_c := Vector3(0, 41.0 / ApeModel.GRID + 15.0 / ApeModel.HEAD_GRID, 0)  # head centre (neck + half the skull)
	for from in [Vector3(0, 1.9, -6.5), Vector3(-6.5, 1.9, 0), Vector3(0, 1.9, 6.5), head_c + Vector3(0.9, 0.5, -1.9)]:
		var cam := Camera3D.new()
		cam.fov = 40
		root3d.add_child(cam)
		cam.look_at_from_position(from, mid if from.length() > 4.0 else head_c)
		_cams.append(cam)
	_cams[0].current = true
	DisplayServer.window_set_size(Vector2i(500, 800))


func _process(_delta: float) -> bool:
	_frames += 1
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
	var sheet := Image.create(w * _shots.size(), h, false, Image.FORMAT_RGB8)
	for i in _shots.size():
		sheet.blit_rect(_shots[i], Rect2i(0, 0, w, h), Vector2i(i * w, 0))
	var abs_path := ProjectSettings.globalize_path(out_path)
	DirAccess.make_dir_recursive_absolute(abs_path.get_base_dir())
	sheet.save_png(abs_path)
	print(JSON.stringify({"ape_sheet": abs_path, "views": _shots.size()}))
	quit(0)
	return false
