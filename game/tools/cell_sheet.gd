extends Node

## One atlas of every cell-built mob model (MobModel + WildModels), each framed to its own size
## from the front-left and from the back-right, so the looks can be judged without a world:
##   godot_console.exe --path game res://tools/cell_sheet.tscn -- out=res://out/mobs/cells.png
## only=fox,wolf renders just those; cols=4 sets the atlas width in models.

const SPECIES := ["sheep", "sheep_shorn", "pig", "chicken", "bear", "deer", "boar", "rabbit", "fox", "wolf", "duck", "frog", "crab", "fish", "bat", "cave_spider"]
const TILE := Vector2i(360, 280)

var out_path := "res://out/mobs/cells.png"
var only := PackedStringArray()
var cols := 4
var _frames := 0
var _list: Array = []
var _models: Array[MobModel] = []
var _cam: Camera3D
var _shots: Array[Image] = []


func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"out": out_path = kv[1]
			"only": only = kv[1].split(",")
			"cols": cols = int(kv[1])
	var root3d := Node3D.new()
	add_child(root3d)
	var x := 0.0
	for s in SPECIES:
		if not only.is_empty() and not s in only:
			continue
		var m := MobModel.create(s.replace("_shorn", "") if s != "cave_spider" else s, s == "sheep_shorn")
		root3d.add_child(m)
		m.position = Vector3(x, 0, 0)
		_models.append(m)
		_list.append(s)
		x += 8.0
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
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
	var ground := MeshInstance3D.new()
	var pm := PlaneMesh.new()
	pm.size = Vector2(x + 20, 20)
	ground.mesh = pm
	ground.position.x = x * 0.5
	var gm := StandardMaterial3D.new()
	gm.albedo_color = Color(0.35, 0.6, 0.3)
	ground.material_override = gm
	root3d.add_child(ground)
	_cam = Camera3D.new()
	_cam.fov = 40
	root3d.add_child(_cam)
	_cam.current = true
	get_window().size = TILE
	DisplayServer.window_set_size(TILE)


## Model bounds in world space (parts are MeshInstance3D children, positioned at their pivots).
func _bounds(m: Node3D) -> AABB:
	var box := AABB()
	var first := true
	for n in m.find_children("*", "MeshInstance3D", true, false):
		var mi: MeshInstance3D = n
		var b := mi.global_transform * mi.get_aabb()
		box = b if first else box.merge(b)
		first = false
	return box


func _process(_delta: float) -> void:
	_frames += 1
	if _frames % 4 != 0:
		return
	var k := _frames / 4 - 1   # which shot was just rendered; two per model (front-left, back-right)
	if k >= 1:
		_shots.append(get_viewport().get_texture().get_image())
	if k >= _models.size() * 2:
		_save()
		return
	var m := _models[k / 2]
	var box := _bounds(m)
	var centre := box.get_center()
	var dist := maxf(box.size.length(), 0.3) * 1.35
	var dir := Vector3(0.7, 0.45, 0.9) if k % 2 == 0 else Vector3(-0.7, 0.45, -0.9)
	_cam.look_at_from_position(centre + dir.normalized() * dist, centre)


func _save() -> void:
	var per := 2
	var rows := int(ceil(_models.size() / float(cols)))
	var sheet := Image.create(TILE.x * cols * per, TILE.y * rows, false, Image.FORMAT_RGB8)
	for i in _shots.size():
		var cell := i / per
		var at := Vector2i(((cell % cols) * per + i % per) * TILE.x, (cell / cols) * TILE.y)
		sheet.blit_rect(_shots[i], Rect2i(Vector2i.ZERO, TILE), at)
	var abs_path := ProjectSettings.globalize_path(out_path)
	DirAccess.make_dir_recursive_absolute(abs_path.get_base_dir())
	sheet.save_png(abs_path)
	print(JSON.stringify({"cell_sheet": abs_path, "models": _list}))
	get_tree().quit(0)
