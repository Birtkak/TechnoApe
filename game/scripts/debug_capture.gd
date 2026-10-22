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
var target := ""      # lake|ruin|spring|river|waterfall|marsh|alpine|snow|mountain|<biome name>: start before the nearest one and fly over it
var offset_m := 40.0  # start this far from the target (along +z)
var pitch := -0.2     # camera pitch in radians (negative = looking down)
var walk := false     # walk (gravity, step-up) instead of flying; reports distance travelled
var mine := false     # stand still, look down and hold break for the whole run; JSON reports picked_up
var place := false    # stand still and place the held item ahead every place_repeat_s; JSON shows the hotbar count drop
var open_inventory := false   # open the inventory screen (for HUD screenshots)
var scatter_items := false    # also drop one item entity per given item in front of the player
var stations := false         # place a crafting table, chest and furnace in front of the player; the furnace gets iron ore + coal
var open_screen := ""          # open=crafting_table|chest|furnace: open that station's screen (needs stations=1)
var give: Array = []  # give=oak_log:12,wooden_pickaxe  -> starting items
var _start_pos := Vector3.ZERO
var _strip: FrameSheet
var _frame_ms := PackedFloat64Array()
var _elapsed := 0.0
var _capture_accum := 0.0
var _main: Node
var _done := false


func _ready() -> void:
	var args: Dictionary = Settings.apply_cmdline()
	autopilot = args.get("autopilot", "0") == "1"
	if autopilot and not args.has("persist"):
		Settings.persist_world = false  # unattended test runs never write world saves
	duration = float(args.get("duration", "20"))
	capture_every = float(args.get("capture_every", "0"))
	out_dir = args.get("out", "res://out")
	altitude_m = float(args.get("alt", "30"))
	speed_m = float(args.get("speed", "6"))
	target = args.get("target", "")
	offset_m = float(args.get("offset", "40"))
	pitch = float(args.get("pitch", "-0.2"))
	walk = args.get("walk", "0") == "1"
	mine = args.get("mine", "0") == "1"
	place = args.get("place", "0") == "1"
	open_inventory = args.get("open_inventory", "0") == "1"
	scatter_items = args.get("scatter_items", "0") == "1"
	stations = args.get("stations", "0") == "1"
	open_screen = args.get("open", "")
	for spec in String(args.get("give", "")).split(",", false):
		var parts := spec.split(":")
		give.append({"id": parts[0], "count": int(parts[1]) if parts.size() > 1 else 1})
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
	if _frame_ms.is_empty():
		_start_pos = _main.player.position
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
	s["mode_requested"] = "walk" if walk else "fly"
	s["travelled_m"] = snappedf(Settings.v2m((_main.player.position - _start_pos).length()), 0.1)
	s["travelled_horizontal_m"] = snappedf(Settings.v2m(Vector2(_main.player.position.x - _start_pos.x, _main.player.position.z - _start_pos.z).length()), 0.1)
	if _main.has_method("verify_bubble"):
		s["bubble_verify"] = _main.verify_bubble()
	s["frames"] = ft.size()
	s["avg_frame_ms"] = snappedf(sum / n, 0.01)
	s["p50_frame_ms"] = snappedf(ft[n / 2] if ft.size() else 0.0, 0.01)
	s["p95_frame_ms"] = snappedf(ft[int(n * 0.95)] if ft.size() else 0.0, 0.01)
	s["p99_frame_ms"] = snappedf(ft[int(n * 0.99)] if ft.size() else 0.0, 0.01)
	s["settings"] = {"seed": Settings.world_seed, "seed_text": Settings.world_seed_text, "render_distance_m": Settings.render_distance_m, "view_distance_m": Settings.view_distance_m,
		"simulation_distance_m": Settings.simulation_distance_m, "lod_count": Settings.lod_count, "sim_tick_hz": Settings.sim_tick_hz}
	print(JSON.stringify(s))
	get_tree().quit(0)
