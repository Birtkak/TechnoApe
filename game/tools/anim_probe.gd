extends SceneTree

## Does the terrain shader animate its water / lava / fire tiles and leave the
## rest alone? Renders one quad per tile (stone, grass, water, lava, fire) with
## the real material, grabs the frame twice 0.5 s apart and reports the share
## of pixels that changed per quad. Not headless.
##   godot_console.exe --path game --script res://tools/anim_probe.gd

const TILES := {"stone": 0, "grass": 2, "water": 5, "lava": 7, "fire": 12}
var _frames := 0
var _first: Image
var _t0 := 0.0
var _quads := {}


func _init() -> void:
	var root3d := Node3D.new()
	get_root().add_child(root3d)
	var atlas: Texture2D = load("res://assets/textures/atlas.png")
	var mat := ShaderMaterial.new()
	mat.shader = load("res://assets/shaders/terrain.gdshader")
	mat.set_shader_parameter("atlas", atlas)
	mat.set_shader_parameter("atlas_tiles", Vector2(GameMaterials.ATLAS_TILES))
	mat.set_shader_parameter("water_tile", 5.0)
	mat.set_shader_parameter("lava_tile", 7.0)
	mat.set_shader_parameter("fire_tile", 12.0)
	var i := 0
	for name in TILES:
		var t: int = TILES[name]
		var st := SurfaceTool.new()
		st.begin(Mesh.PRIMITIVE_TRIANGLES)
		var u0 := float(t % GameMaterials.ATLAS_TILES.x) / GameMaterials.ATLAS_TILES.x
		var v0 := float(t / GameMaterials.ATLAS_TILES.x) / GameMaterials.ATLAS_TILES.y
		var du := 1.0 / GameMaterials.ATLAS_TILES.x
		var dv := 1.0 / GameMaterials.ATLAS_TILES.y
		var p := [Vector3(0, 0, 0), Vector3(1, 0, 0), Vector3(1, 1, 0), Vector3(0, 1, 0)]
		var uv := [Vector2(u0, v0 + dv), Vector2(u0 + du, v0 + dv), Vector2(u0 + du, v0), Vector2(u0, v0)]
		for k in [0, 2, 1, 0, 3, 2]:
			st.set_normal(Vector3(0, 0, 1))
			st.set_uv(uv[k])
			st.set_color(Color.WHITE)
			st.add_vertex(p[k])
		var mi := MeshInstance3D.new()
		mi.mesh = st.commit()
		mi.material_override = mat
		mi.position = Vector3(i * 1.2, 0, 0)
		root3d.add_child(mi)
		_quads[name] = i
		i += 1
	var cam := Camera3D.new()
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = 1.6
	cam.look_at_from_position(Vector3((i - 1) * 0.6 + 0.5, 0.5, 3), Vector3((i - 1) * 0.6 + 0.5, 0.5, 0))
	root3d.add_child(cam)
	cam.current = true
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color.BLACK
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color.WHITE
	e.ambient_light_energy = 1.0
	env.environment = e
	root3d.add_child(env)
	DisplayServer.window_set_size(Vector2i(1200, 400))


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 5:
		_first = get_root().get_viewport().get_texture().get_image()
		_t0 = Time.get_ticks_msec() / 1000.0
	elif _first and Time.get_ticks_msec() / 1000.0 - _t0 >= 0.5:
		var second := get_root().get_viewport().get_texture().get_image()
		var w := _first.get_width()
		var h := _first.get_height()
		var n := _quads.size()
		var report := {"test": "anim_probe", "dt_s": snappedf(Time.get_ticks_msec() / 1000.0 - _t0, 0.01)}
		for name in _quads:
			var qi: int = _quads[name]
			var x0 := int(w * float(qi) / n)
			var x1 := int(w * float(qi + 1) / n)
			var changed := 0
			var total := 0
			for y in range(0, h, 2):
				for x in range(x0, x1, 2):
					var a := _first.get_pixel(x, y)
					var b := second.get_pixel(x, y)
					if a.a == 0 and b.a == 0:
						continue
					if a.r + a.g + a.b < 0.02 and b.r + b.g + b.b < 0.02:
						continue  # background
					total += 1
					if absf(a.r - b.r) + absf(a.g - b.g) + absf(a.b - b.b) > 0.1:
						changed += 1
			report[name + "_changed_pct"] = snappedf(100.0 * changed / maxi(1, total), 0.1)
		var dir := ProjectSettings.globalize_path("res://out/anim")
		DirAccess.make_dir_recursive_absolute(dir)
		_first.save_png(dir + "/a.png")
		second.save_png(dir + "/b.png")
		print(JSON.stringify(report))
		quit(0)
	return false
