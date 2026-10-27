extends Node

## Renders the 1 m station models (crafting table, chest, furnace, lit furnace) and a few work
## pieces (WorkPalette, shaded) in a row and saves one PNG, so their size and look can be checked
## next to each other without a world:
##   godot_console.exe --path game res://tools/station_sheet.tscn -- out=res://out/stations.png
## (a scene, not --script: the models read the Settings autoload.) Not headless; quits after the frame is saved.

var out_path := "res://out/stations.png"
var _frames := 0
var _names: Array[String] = []   # one tile per model
var _cam: Camera3D
var _sheet: Image
const TILE := 360
const COLS := 8
const STEP := 4.0   # metres between models
const DEADLINE_MS := 20000   # a stuck frame never keeps the process alive past this


func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "out":
			out_path = kv[1]
	var root3d := Node3D.new()
	add_child(root3d)
	var i := 0
	for name in ["crafting_table", "chest", "furnace", "furnace_lit"]:
		var mi := MeshInstance3D.new()
		mi.mesh = StationEntity.model_mesh(name)
		mi.scale = Vector3.ONE / Settings.BLOCK * 0.5   # voxel units -> metres
		mi.position = Vector3(i * STEP, 0, 0)
		root3d.add_child(mi)
		_names.append(name)
		i += 1
	# the real work pieces, fresh from their entities, and the furniture
	var items := ItemRegistry.load_default()
	var store := BlockEntities.new(items, RecipeRegistry.load_default(items))
	for kind in ["knap", "clayform", "ground", "firepit", "sieve", "firewood_pile", "assembly", "anvil", "bronze_anvil", "forge", "bloomery", "grave", "earth_oven", "rack", "water_work", "stroke"]:
		var e: RefCounted = store.create(Vector3i(0, 0, 0), kind)
		if e == null or not e.has_method("view_parts"):
			continue
		var parts: Dictionary = e.view_parts()
		var v := WorkPalette.make_view(Vector3i.ZERO, parts.get("solid", {}), parts.get("ghost", {}), parts.get("guide", {}), parts.get("laid", []))
		v.scale = Vector3.ONE * 0.5
		v.position = Vector3(i * STEP + 0.25, 0, 0.25)
		root3d.add_child(v)
		_names.append(kind)
		i += 1
	for kind in FurnitureEntity.KINDS:
		var mi := MeshInstance3D.new()
		mi.mesh = FurnitureEntity.mesh_for(kind)
		mi.material_override = FurnitureEntity.material()
		mi.scale = Vector3.ONE * 0.5
		mi.position = Vector3(i * STEP + 0.5, 0, 0.5)
		root3d.add_child(mi)
		_names.append(kind)
		i += 1
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
	sun.light_energy = 1.2
	root3d.add_child(sun)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.55, 0.7, 0.9)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.6, 0.6, 0.65)
	e.ambient_light_energy = 0.8
	env.environment = e
	root3d.add_child(env)
	_cam = Camera3D.new()
	_cam.fov = 45
	root3d.add_child(_cam)
	_cam.current = true
	_aim(0)
	get_tree().root.size = Vector2i(TILE, TILE)
	DisplayServer.window_set_size(Vector2i(TILE, TILE))
	_sheet = Image.create(COLS * TILE, ceili(float(_names.size()) / COLS) * TILE, false, Image.FORMAT_RGBA8)


## Looks at model `k` from the front-right, a little above (models stand on 0..1 m at x = k * STEP).
func _aim(k: int) -> void:
	var at := Vector3(k * STEP + 0.5, 0.35, 0.5)
	_cam.look_at_from_position(at + Vector3(1.6, 1.3, 2.2), at)


func _process(_delta: float) -> void:
	if Time.get_ticks_msec() > DEADLINE_MS:
		push_error("station_sheet: deadline hit after %d tiles" % (_frames / 3))
		get_tree().quit(2)
	_frames += 1
	if _frames % 3 != 0:
		return
	var k := _frames / 3 - 1   # the tile the last frames drew
	var img := get_viewport().get_texture().get_image()
	img.convert(Image.FORMAT_RGBA8)
	_sheet.blit_rect(img, Rect2i(0, 0, TILE, TILE), Vector2i(k % COLS * TILE, k / COLS * TILE))
	if k + 1 < _names.size():
		_aim(k + 1)
		return
	var abs_path := ProjectSettings.globalize_path(out_path)
	DirAccess.make_dir_recursive_absolute(abs_path.get_base_dir())
	_sheet.save_png(abs_path)
	print(JSON.stringify({"station_sheet": abs_path, "models": _names}))
	get_tree().quit(0)
