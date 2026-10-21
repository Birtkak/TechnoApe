extends SceneTree

# P1: render a trivial 3D scene from the CLI and save a PNG so Claude can
# look at it. Needs a real renderer, so run WITHOUT --headless:
#   godot_console.exe --path . --script res://screenshot.gd --resolution 640x360
# Exit 0 = PNG written, 1 = failure.

const OUT_PATH := "res://out/screenshot.png"
var _frames := 0


func _initialize() -> void:
	var world := Node3D.new()
	world.name = "World"
	root.add_child(world)

	var cam := Camera3D.new()
	cam.position = Vector3(3, 2.5, 4)
	cam.look_at_from_position(cam.position, Vector3.ZERO, Vector3.UP)
	world.add_child(cam)
	cam.make_current()

	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
	world.add_child(sun)

	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.45, 0.65, 0.9)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.4, 0.4, 0.45)
	env.environment = e
	world.add_child(env)

	# A small 3x1x3 patch of coloured "blocks" so the picture is unmistakable.
	for x in range(-1, 2):
		for z in range(-1, 2):
			var m := MeshInstance3D.new()
			m.mesh = BoxMesh.new()
			m.position = Vector3(x, 0, z)
			var mat := StandardMaterial3D.new()
			mat.albedo_color = Color(0.3 + 0.3 * (x + 1), 0.6, 0.3 + 0.3 * (z + 1))
			m.material_override = mat
			world.add_child(m)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < 4:
		return false
	var img := root.get_viewport().get_texture().get_image()
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://out"))
	var err := img.save_png(OUT_PATH)
	print("P1 screenshot: %s %dx%d err=%d" % [ProjectSettings.globalize_path(OUT_PATH), img.get_width(), img.get_height(), err])
	quit(0 if err == OK else 1)
	return true
