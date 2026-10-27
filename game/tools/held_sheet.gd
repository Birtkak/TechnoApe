extends Node

## Renders every item as it looks in the first-person hand (the real Viewmodel under a game
## camera), one cell per item with its id, tiled into pages out/held/held_N.png:
##   godot_console.exe --path game res://tools/held_sheet.tscn -- out=res://out/held
## Not headless (needs the renderer); quits when the last page is saved.

const W := 640
const H := 360
var CROP := Rect2i(220, 60, 420, 300)   # the lower right of the view, where the held item sits
const COLS := 6
const ROWS := 5

var out_dir := "res://out/held"
var _ids: Array = []
var _vm: Viewmodel
var _label: Label
var _i := -1
var _wait := 0
var _page: Image
var _page_n := 0


func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "out":
			out_dir = kv[1]
	var only := ""   # only=pickaxe,axe: ids containing any of these
	for a in OS.get_cmdline_user_args():
		if a == "full=1":   # the whole view, to check the pose
			CROP = Rect2i(0, 0, W, H)
		if a.begins_with("only="):
			only = a.substr(5)
	var items := ItemRegistry.load_default()
	_ids = items.items.keys()
	for id in ItemSprites.get_default().sprites:
		if not _ids.has(id):
			_ids.append(id)
	if only != "":
		_ids = _ids.filter(func(id: String) -> bool: return Array(only.split(",")).any(func(w: String) -> bool: return id.contains(w)))
	_ids.sort()
	get_window().size = Vector2i(W, H)
	DisplayServer.window_set_size(Vector2i(W, H))
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.55, 0.7, 0.9)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.6, 0.6, 0.65)
	e.ambient_light_energy = 0.8
	env.environment = e
	add_child(env)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
	sun.light_energy = 1.2
	add_child(sun)
	var cam := Camera3D.new()
	cam.fov = Settings.fov
	add_child(cam)
	cam.current = true
	# shortcut: plain atlas material for block cubes, not the terrain shader (no AO tint or water scroll)
	var atlas := StandardMaterial3D.new()
	atlas.albedo_texture = load("res://assets/textures/atlas.png")
	atlas.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	_vm = Viewmodel.new()
	_vm.items = items
	_vm.atlas_material = atlas
	cam.add_child(_vm)
	var layer := CanvasLayer.new()
	add_child(layer)
	_label = Label.new()
	_label.position = Vector2(CROP.position) + Vector2(6, 2)
	_label.add_theme_font_size_override("font_size", 18)
	_label.add_theme_color_override("font_outline_color", Color.BLACK)
	_label.add_theme_constant_override("outline_size", 5)
	layer.add_child(_label)
	_next()


func _next() -> void:
	_i += 1
	if _i < _ids.size():
		_vm.show_item({"id": _ids[_i], "count": 1})
		_label.text = "%d %s" % [_i, _ids[_i]]
	_wait = 3   # the threaded renderer lags a frame or two behind


func _process(_delta: float) -> void:
	_wait -= 1
	if _wait > 0:
		return
	if _i >= _ids.size():
		_save_page()
		var f := FileAccess.open("%s/ids.json" % out_dir, FileAccess.WRITE)   # for the phone page (cell order)
		f.store_string(JSON.stringify(_ids))
		f.close()
		print(JSON.stringify({"tool": "held_sheet", "items": _ids.size(), "pages": _page_n}))
		get_tree().quit(0)
		return
	var slot := _i % (COLS * ROWS)
	if slot == 0:
		_save_page()
		_page = Image.create(COLS * CROP.size.x, ROWS * CROP.size.y, false, Image.FORMAT_RGBA8)
	var frame := get_viewport().get_texture().get_image()
	frame.convert(Image.FORMAT_RGBA8)
	_page.blit_rect(frame, CROP, Vector2i((slot % COLS) * CROP.size.x, (slot / COLS) * CROP.size.y))
	_next()


func _save_page() -> void:
	if _page == null:
		return
	var path := ProjectSettings.globalize_path("%s/held_%d.png" % [out_dir, _page_n])
	DirAccess.make_dir_recursive_absolute(path.get_base_dir())
	_page.save_png(path)
	print("saved ", path)
	_page_n += 1
	_page = null
