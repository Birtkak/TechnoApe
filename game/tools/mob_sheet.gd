extends Node

## Renders the animals and furniture of ADR 0022 without a world, for checking
## the models: a row of sheep, sheared sheep, lamb, pig and piglet, plus a bed
## and a chair, from the front-left, then the whole line from above.
##   godot_console.exe --path game res://tools/mob_sheet.tscn -- out=res://out/mobs/sheet.png
## graze=1 poses the animals grazing, walk=0.5 mid-stride.
## Everything is at the game's scale (Mob._apply_size, FurnitureEntity.make_view) and every camera is
## aimed from the model's measured bounds, so the sheet stays framed when sizes change.

const FOV := 40.0
const GAP := 16.0    # voxels between neighbours in the line (two sheep widths keep close-ups clean)

var out_path := "res://out/mobs/sheet.png"
var _frames := 0
var _shots: Array[Image] = []
var _cams: Array[Camera3D] = []
var _models: Array[MobModel] = []
var _subjects: Array[Node3D] = []
var _root: Node3D
var _graze := 0.0
var _walk := 0.0
var _species := PackedStringArray()


func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"out": out_path = kv[1]
			"graze": _graze = float(kv[1])
			"walk": _walk = float(kv[1])
			"species": _species = kv[1].split(",")   # species=deer,crow,gloam_hound: those instead of the farm row
	_root = Node3D.new()
	add_child(_root)
	var row := [["sheep", false, 1.0], ["sheep", true, 1.0], ["sheep", false, 0.5], ["pig", false, 1.0], ["pig", false, 0.5]]
	if not _species.is_empty():
		row = Array(_species).map(func(s: String) -> Array: return [s, false, 1.0])
	for e in row:
		var m := MobModel.create(e[0], e[1])
		_root.add_child(m)
		m.scale = Vector3.ONE * e[2] * MobModel.UNIT
		_models.append(m)
		_subjects.append(m)
	for k in ["bed", "chair"]:
		var v := FurnitureEntity.new(Vector3i.ZERO, k, 0).make_view()
		_root.add_child(v)
		_subjects.append(v)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
	sun.light_energy = 1.1
	_root.add_child(sun)
	var env := WorldEnvironment.new()
	var e2 := Environment.new()
	e2.background_mode = Environment.BG_COLOR
	e2.background_color = Color(0.5, 0.65, 0.85)
	e2.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e2.ambient_light_color = Color(0.7, 0.7, 0.75)
	e2.ambient_light_energy = 0.9
	env.environment = e2
	_root.add_child(env)
	DisplayServer.window_set_size(Vector2i(480, 360))


## The world-space bounds of every mesh under n.
static func _bounds(n: Node) -> AABB:
	var box := AABB()
	var first := true
	for mi: MeshInstance3D in n.find_children("*", "MeshInstance3D", true, false):
		if mi.mesh == null:
			continue
		var b := mi.global_transform * mi.get_aabb()
		box = b if first else box.merge(b)
		first = false
	return box


## Lines the subjects up along x by their measured widths, feet on y = 0, and aims a camera at each
## (3/4 from the front: the models face +Z) at a distance that fits its bounding sphere, then one
## over the whole line.
func _layout() -> void:
	for m in _models:
		if m.scale.x < MobModel.UNIT:
			m.head.scale = Vector3.ONE * 1.45   # babies' big heads (Mob._apply_size)
	var x := 0.0
	var all := AABB()
	for i in _subjects.size():
		var s := _subjects[i]
		var b := _bounds(s)
		s.position += Vector3(x - b.position.x, -b.position.y, -b.get_center().z)
		x += b.size.x + GAP
	for i in _subjects.size():
		var b := _bounds(_subjects[i])
		all = b if i == 0 else all.merge(b)
		var r := b.size.length() * 0.5
		var dist := r / sin(deg_to_rad(FOV * 0.5)) * 1.05
		var cam := Camera3D.new()
		cam.fov = FOV
		cam.far = dist * 4.0 + 100.0
		_root.add_child(cam)
		cam.look_at_from_position(b.get_center() + Vector3(0.55, 0.45, 0.7).normalized() * dist, b.get_center())
		_cams.append(cam)
	var ground := MeshInstance3D.new()
	var pm := PlaneMesh.new()
	pm.size = Vector2(all.size.x + GAP * 4.0, all.size.z + GAP * 6.0)
	ground.mesh = pm
	ground.position = Vector3(all.get_center().x, 0, all.get_center().z)
	var gm := StandardMaterial3D.new()
	gm.albedo_color = Color(0.35, 0.6, 0.3)
	ground.material_override = gm
	_root.add_child(ground)
	var top := Camera3D.new()
	top.fov = 55
	var span := all.size.x * 0.5 / tan(deg_to_rad(55.0 * 0.5)) / 1.333 * 1.15   # the 4:3 window's width
	top.far = span * 4.0 + 100.0
	_root.add_child(top)
	top.look_at_from_position(all.get_center() + Vector3(0, span * 0.85, span * 0.55), all.get_center())
	_cams.append(top)
	_cams[0].current = true


func _process(delta: float) -> void:
	_frames += 1
	if _frames == 1:
		_layout()
	for m in _models:
		m.animate(delta, 0.0 if _walk <= 0.0 else 3.0, _graze, 0.3, 0.0, 0.0)
		if _walk > 0.0:
			for i in 4:
				m.legs[i].rotation.x = (0.6 if i in [0, 3] else -0.6) * _walk
	if _frames % 4 != 0:
		return
	var k := _frames / 4 - 1
	if k >= 1:
		_shots.append(get_viewport().get_texture().get_image())
	if k < _cams.size():
		_cams[k].current = true
		return
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
	get_tree().quit(0)
