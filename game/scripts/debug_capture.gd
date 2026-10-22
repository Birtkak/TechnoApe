extends Node

## Autoload: unattended runs for Claude.
##   godot --path game -- autopilot=1 duration=20 capture_every=1 out=res://out/run1
## Flies the player forward, captures a filmstrip, prints one JSON line with
## frame-time percentiles, terrain and sim stats, then quits with code 0.
## Without autopilot it only enables `screenshot=1` (save a PNG then continue).

var autopilot := false
var duration := 20.0
var capture_every := 0.0
var out_dir := "res://out"
var altitude_m := 30.0
var speed_m := 6.0
var target := ""      # "lake" | "ruin": start the flight before the nearest one and fly over it
var offset_m := 40.0  # start this far from the target (along +z)
var pitch := -0.2     # camera pitch in radians (negative = looking down)
var _strip: FrameSheet
var _frame_ms := PackedFloat64Array()
var _elapsed := 0.0
var _capture_accum := 0.0
var _main: Node
var _done := false


func _ready() -> void:
	var args: Dictionary = Settings.apply_cmdline()
	autopilot = args.get("autopilot", "0") == "1"
	duration = float(args.get("duration", "20"))
	capture_every = float(args.get("capture_every", "0"))
	out_dir = args.get("out", "res://out")
	altitude_m = float(args.get("alt", "30"))
	speed_m = float(args.get("speed", "6"))
	target = args.get("target", "")
	offset_m = float(args.get("offset", "40"))
	pitch = float(args.get("pitch", "-0.2"))
	if capture_every > 0.0:
		_strip = FrameSheet.new(320, 180, 5)
	if args.get("vsync", "1") == "0":
		DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	set_process(autopilot)


func attach(main: Node) -> void:
	_main = main


func _process(delta: float) -> void:
	if _done or _main == null:
		return
	_elapsed += delta
	if _elapsed < 1.0:
		return  # let the world load
	_frame_ms.push_back(delta * 1000.0)
	if _strip:
		_capture_accum += delta
		if _capture_accum >= capture_every:
			_capture_accum -= capture_every
			_strip.add(get_viewport())
	if _elapsed >= duration + 1.0:
		_finish()


func _finish() -> void:
	_done = true
	var ft := _frame_ms.duplicate()
	ft.sort()
	var n := maxi(1, ft.size())
	var sum := 0.0
	for v in ft:
		sum += v
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(out_dir))
	get_viewport().get_texture().get_image().save_png(out_dir + "/final.png")
	if _strip and _strip.count() > 0:
		_strip.save(out_dir + "/strip.png")
	var s: Dictionary = _main.snapshot()
	s["test"] = "autopilot"
	if _main.has_method("verify_bubble"):
		s["bubble_verify"] = _main.verify_bubble()
	s["frames"] = ft.size()
	s["avg_frame_ms"] = snappedf(sum / n, 0.01)
	s["p50_frame_ms"] = snappedf(ft[n / 2] if ft.size() else 0.0, 0.01)
	s["p95_frame_ms"] = snappedf(ft[int(n * 0.95)] if ft.size() else 0.0, 0.01)
	s["p99_frame_ms"] = snappedf(ft[int(n * 0.99)] if ft.size() else 0.0, 0.01)
	s["settings"] = {"render_distance_m": Settings.render_distance_m, "view_distance_m": Settings.view_distance_m,
		"simulation_distance_m": Settings.simulation_distance_m, "lod_count": Settings.lod_count, "sim_tick_hz": Settings.sim_tick_hz}
	print(JSON.stringify(s))
	get_tree().quit(0)
