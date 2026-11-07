extends Node

## Contact sheet for ADR 0164: a wall with a sign, a pinned note and a trading post in the world, then the
## sign's text screen, the owner's trading screen and the customer's. Needs a window (not headless):
##   Godot_console.exe --path game res://tools/left_things_sheet.tscn --resolution 960x540 --position -30000,-30000 -- out=res://out/left_things.png
## Runs the Settings autoload (a scene, not --script). Quits when the sheet is saved or after DEADLINE_MS.

const DEADLINE_MS := 40000
var out_path := "res://out/left_things.png"
var _shots: Array[Image] = []
var _world: Node3D
var _cam: Camera3D
var _t0 := 0


func _ready() -> void:
	_t0 = Time.get_ticks_msec()
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "out":
			out_path = kv[1]
	_run.call_deferred()


func _frames(n: int) -> void:
	for i in n:
		await get_tree().process_frame


func _snap() -> void:
	await _frames(4)
	var img := get_viewport().get_texture().get_image()
	img.convert(Image.FORMAT_RGBA8)
	_shots.append(img)


func _run() -> void:
	var items := ItemRegistry.load_default()
	var recipes := RecipeRegistry.load_default(items)
	# ---- the world part ----
	_world = Node3D.new()
	add_child(_world)
	var env := WorldEnvironment.new()
	env.environment = Environment.new()
	env.environment.background_mode = Environment.BG_COLOR
	env.environment.background_color = Color(0.55, 0.72, 0.9)
	env.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.environment.ambient_light_color = Color(0.75, 0.75, 0.8)
	env.environment.ambient_light_energy = 0.8
	_world.add_child(env)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-35, 160, 0)
	_world.add_child(sun)
	var mat := StandardMaterial3D.new()
	mat.albedo_color = Color(0.42, 0.55, 0.3)
	_box(Vector3(-20, -1, -20), Vector3(80, 1, 60), mat)
	var wall := StandardMaterial3D.new()
	wall.albedo_color = Color(0.62, 0.6, 0.56)
	_box(Vector3(-4, 0, 9), Vector3(48, 22, 5), wall)
	var store := BlockEntities.new(items, recipes)
	var s1 := store.create_furniture(Vector3i(4, 8, 8), "sign", 0)
	s1.set_text("Mine ->\nGold below\nbring a torch\n- Ann")
	var s2 := store.create_furniture(Vector3i(16, 8, 8), "sign", 0)
	s2.set_text("TRADING POST\nfresh planks\n2 iron each")
	var n1 := store.create_furniture(Vector3i(28, 8, 8), "note", 0)
	n1.set_text("Bob, key is under\nthe flat stone.\nDo not tell Carl")
	for e in [s1, s2, n1]:
		var v: Node3D = e.make_view()
		_world.add_child(v)
	var post := store.create(Vector3i(26, 0, 0), "trading_post") as TradingPostEntity
	post.owner_name = "Ann"
	post.inv.set_slot(0, {"id": "oak_planks", "count": 4})
	post.inv.set_slot(1, {"id": "iron_ingot", "count": 2})
	post.inv.set_slot(2, {"id": "cobblestone", "count": 16})
	post.inv.set_slot(3, {"id": "iron_ingot", "count": 1})
	post.inv.set_slot(TradingPostEntity.STOCK_FROM, {"id": "oak_planks", "count": 40})
	post.inv.set_slot(TradingPostEntity.STOCK_FROM + 1, {"id": "cobblestone", "count": 64})
	post.inv.set_slot(TradingPostEntity.TAKE_FROM, {"id": "iron_ingot", "count": 6})
	var parts: Dictionary = post.view_parts()
	_world.add_child(WorkPalette.make_view(post.pos, parts.get("solid", {})))
	_cam = Camera3D.new()
	_cam.fov = 55
	_world.add_child(_cam)
	_cam.current = true
	_cam.position = Vector3(16, 11, -30)
	_cam.look_at(Vector3(16, 8, 9))
	await _snap()   # 0: the wall, from 5 m
	_cam.position = Vector3(9, 9.5, -9)   # a sign close up
	_cam.look_at(Vector3(10, 9, 9))
	await _snap()   # 1
	_cam.position = Vector3(30, 9.5, -14)   # the note and the stall
	_cam.look_at(Vector3(28, 6, 4))
	await _snap()   # 2
	_world.visible = false
	# ---- the screens ----
	var pi := PlayerInventory.new(items, recipes)
	pi.inv.add("iron_ingot", 12)
	pi.inv.add("oak_planks", 3)
	pi.inv.add("note", 1, {"text": "hello"})
	var screen := InventoryScreen.new()
	add_child(screen)
	screen.setup(pi)
	if screen.browser_view:
		screen.browser_view.visible = false
	screen.open_container(post)
	screen.browser_view.visible = false
	await _snap()   # 3: the owner's screen
	screen.close()
	screen.open_container(TradeView.new(post, func(_k: int) -> void: pass))
	screen.browser_view.visible = false
	await _snap()   # 4: the customer's
	screen.close()
	var ts := TextScreen.new()
	add_child(ts)
	ts.open("Sign", "Mine ->\nGold below\nbring a torch\n- Ann", SignText.SIGN_LINES, SignText.SIGN_COLS)
	await _snap()   # 5: the text screen
	_save()


func _box(pos: Vector3, size: Vector3, m: Material) -> void:
	var mi := MeshInstance3D.new()
	var b := BoxMesh.new()
	b.size = size
	mi.mesh = b
	mi.material_override = m
	mi.position = pos + size / 2.0
	_world.add_child(mi)


func _save() -> void:
	var w := _shots[0].get_width()
	var h := _shots[0].get_height()
	var sheet := Image.create(w * 2, h * 3, false, Image.FORMAT_RGBA8)
	for i in _shots.size():
		sheet.blit_rect(_shots[i], Rect2i(0, 0, w, h), Vector2i((i % 2) * w, (i / 2) * h))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(out_path.get_base_dir()))
	sheet.save_png(out_path)
	print("left_things_sheet: saved ", ProjectSettings.globalize_path(out_path))
	get_tree().quit()


func _process(_d: float) -> void:
	if Time.get_ticks_msec() - _t0 > DEADLINE_MS:
		push_error("left_things_sheet: deadline")
		get_tree().quit(1)
