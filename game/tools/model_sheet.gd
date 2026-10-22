extends SceneTree

## Renders every micro-voxel model (materials.gd "micro" kinds) in a row on a
## grass slab and saves one PNG, so the shapes can be checked without a world:
##   godot_console.exe --path game --script res://tools/model_sheet.gd -- out=res://out/models.png
## Not headless (needs the renderer); quits after the frame is saved.

var out_path := "res://out/models.png"
var _frames := 0


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "out":
			out_path = kv[1]
	var root3d := Node3D.new()
	get_root().add_child(root3d)
	var atlas: Texture2D = load("res://assets/textures/atlas.png")
	var mat := StandardMaterial3D.new()
	mat.albedo_texture = atlas
	mat.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST_WITH_MIPMAPS
	mat.roughness = 1.0
	var kinds: Array = []
	for d in GameMaterials.defs():
		if d.has("micro"):
			kinds.append(d)
	var g := GameMaterials.MICRO_GRID
	var tiles := GameMaterials.micro_paint_tiles()
	var i := 0
	for d in kinds:
		var cells: PackedInt32Array = GameMaterials.micro_pattern(d["micro"], d["tiles"])
		var mesh: ArrayMesh = MicroMesher.build(cells, Vector3i(g, g, g), g, tiles, GameMaterials.ATLAS_TILES, Vector3.ZERO)
		var mi := MeshInstance3D.new()
		mi.mesh = mesh
		mi.material_override = mat
		mi.position = Vector3(i * 1.8, 0, 0)
		root3d.add_child(mi)
		# a grass voxel under it, built with the same mesher
		var ground := MicroMesher.fill_box(PackedInt32Array(), Vector3i(g, g, g), Vector3i.ZERO, Vector3i(g, g, g), 2 + 1)
		var gi := MeshInstance3D.new()
		gi.mesh = MicroMesher.build(ground, Vector3i(g, g, g), g, tiles, GameMaterials.ATLAS_TILES, Vector3.ZERO)
		gi.material_override = mat
		gi.position = Vector3(i * 1.8, -1, 0)
		root3d.add_child(gi)
		var label := Label3D.new()
		label.text = d["name"]
		label.font_size = 40
		label.pixel_size = 0.004
		label.position = Vector3(i * 1.8 + 0.5, 1.3, 0.5)
		root3d.add_child(label)
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
	var cam := Camera3D.new()
	var mid := (i - 1) * 1.8 * 0.5 + 0.5
	cam.look_at_from_position(Vector3(mid, 1.8, 5.6), Vector3(mid, 0.3, 0.5))
	cam.fov = 60
	root3d.add_child(cam)
	cam.current = true
	get_root().size = Vector2i(1800, 500)
	DisplayServer.window_set_size(Vector2i(1800, 500))


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 6:
		var img := get_root().get_viewport().get_texture().get_image()
		var abs_path := ProjectSettings.globalize_path(out_path)
		DirAccess.make_dir_recursive_absolute(abs_path.get_base_dir())
		img.save_png(abs_path)
		print(JSON.stringify({"model_sheet": abs_path, "models": get_root().get_child(0).get_child_count()}))
		quit(0)
	return false
