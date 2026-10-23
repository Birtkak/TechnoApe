extends SceneTree

## Renders every item as the game draws it and saves one PNG: the inventory
## icon (ItemIcon) of each item in items.json and each sprite, then a row of
## dropped-item meshes (BlockMeshes.sprite) under a sun, so the looks can be
## checked without playing:
##   godot_console.exe --path game --script res://tools/item_sheet.gd -- out=res://out/items.png
## Not headless (needs the renderer); quits after the frame is saved.

const ICON := 40
const COLS := 24
const MESH_IDS := ["copper_pickaxe", "iron_axe", "bronze_saw", "water_bucket", "raw_meat", "flint", "copper_ingot", "bricks", "glass", "spell_fire_bolt", "torch", "loose_copper_ore"]

var out_path := "res://out/items.png"
var _frames := 0


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "out":
			out_path = kv[1]
	var items := ItemRegistry.load_default()
	var sprites := ItemSprites.get_default()
	var ids: Array = items.items.keys()
	for id in sprites.sprites:
		if not ids.has(id):
			ids.append(id)
	# 3D: dropped-item meshes on a strip, lit like the world
	var root3d := Node3D.new()
	get_root().add_child(root3d)
	for i in MESH_IDS.size():
		var mi := MeshInstance3D.new()
		mi.mesh = BlockMeshes.sprite(MESH_IDS[i])
		mi.position = Vector3(i * 1.3, 0, 0)
		mi.rotation_degrees = Vector3(0, -25 if i % 2 else 25, 0)
		root3d.add_child(mi)
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
	var mid := (MESH_IDS.size() - 1) * 1.3 * 0.5
	cam.look_at_from_position(Vector3(mid, 2.2, 9.5), Vector3(mid, 2.4, 0))
	cam.fov = 45
	root3d.add_child(cam)
	cam.current = true
	# 2D: icon grid over the top part of the window
	var rows := int(ceil(ids.size() / float(COLS)))
	var layer := CanvasLayer.new()
	get_root().add_child(layer)
	var bg := ColorRect.new()
	bg.color = Color(0.22, 0.22, 0.26)
	bg.size = Vector2(COLS * (ICON + 6) + 6, rows * (ICON + 6) + 6)
	layer.add_child(bg)
	for i in ids.size():
		var slot := ColorRect.new()
		slot.color = Color(0.14, 0.14, 0.17)
		slot.position = Vector2(6 + (i % COLS) * (ICON + 6), 6 + (i / COLS) * (ICON + 6))
		slot.size = Vector2(ICON, ICON)
		layer.add_child(slot)
		var icon := ItemIcon.new()
		icon.items = items
		icon.position = Vector2(2, 2)
		icon.size = Vector2(ICON - 4, ICON - 4)
		slot.add_child(icon)
		icon.stack = {"id": ids[i], "count": 1}
	var size := Vector2i(int(bg.size.x), int(bg.size.y) + 260)
	get_root().size = size
	DisplayServer.window_set_size(size)
	print(JSON.stringify({"tool": "item_sheet", "items": ids.size(), "sprites": sprites.sprites.size(), "errors": sprites.errors}))


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 6:
		var img := get_root().get_viewport().get_texture().get_image()
		var abs_path := ProjectSettings.globalize_path(out_path)
		DirAccess.make_dir_recursive_absolute(abs_path.get_base_dir())
		img.save_png(abs_path)
		print("saved ", abs_path)
		return true
	return false
