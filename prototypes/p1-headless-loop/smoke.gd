extends SceneTree

# Smoke test: proves Godot runs headless from the CLI and that exit codes
# and stdout reach the caller. Exit code 0 = pass, 1 = fail.

func _init() -> void:
	var noise := FastNoiseLite.new()
	noise.seed = 42
	var a := noise.get_noise_2d(10.0, 20.0)
	var b := noise.get_noise_2d(10.0, 20.0)
	var ok := is_equal_approx(a, b) and a != 0.0
	print("P1 smoke: godot=%s noise_sample=%f deterministic=%s" % [Engine.get_version_info().string, a, ok])
	quit(0 if ok else 1)
