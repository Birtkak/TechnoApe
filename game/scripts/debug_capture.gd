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
var target := ""      # lake|ruin|spring|river|waterfall|marsh|alpine|snow|mountain|cave|<biome name>: start before the nearest one and fly over it
var offset_m := 40.0  # start this far from the target (along +z)
var inside := false   # start at the target position itself (cave|...) with a debug torch, instead of above it
var third_person := 0 # 1 = camera behind the ape, 2 = in front (F5 views)
var pitch := -0.2     # camera pitch in radians (negative = looking down)
var walk := false     # walk (gravity, step-up) instead of flying; reports distance travelled
var mine := false     # stand still, look down and hold break for the whole run; JSON reports picked_up
var place := false    # stand still and place the held item ahead every place_repeat_s; JSON shows the hotbar count drop
var open_inventory := false   # open the inventory screen (for HUD screenshots)
var scatter_items := false    # also drop one item entity per given item in front of the player
var stations := false         # place a crafting table, chest and furnace in front of the player; the furnace gets iron ore + coal
var open_screen := ""          # open=crafting_table|chest|furnace: open that station's screen (needs stations=1)
var give: Array = []  # give=oak_log:12,wooden_pickaxe  -> starting items
var blast_m := 0.0    # blast=R: mass-edit stress (ADR 0012): craters of radius R m on the ground around the player
var blast_count := 1  # blast_count=N craters per volley (1000 TNT = many small ones)
var blast_every := 2.0  # blast_every=S seconds between volleys, first one 2 s after load
var blast_material := 0  # blast_material=water|sand|...: fill instead of carve (liquid spills: remesh stress)
var _blast_accum := 0.0
var _blast_i := 0
var _blast_centres: Array[Vector3i] = []   # first craters: the JSON reports what the terrain holds there
var _start_pos := Vector3.ZERO
var _strip: FrameSheet
var _frame_ms := PackedFloat64Array()   # frames after load, without capture stalls (what ADR 0009 budgets)
var _load_max_ms := 0.0                 # worst frame before the bubble finished loading
var _loaded_at := -1.0                  # seconds into the run when the world had settled (see _world_settled)
var _settled := false                   # false: the cap hit first (fast flight streams forever)
var _idle_frames := 0
var _last_frame_us := 0
const SETTLE_FRAMES := 30               # voxel tasks idle this many frames in a row = loaded
const SETTLE_CAP_S := 10.0
var _skip_next := false                 # the frame after a filmstrip readback carries its GPU stall
var _capture_stalls := PackedFloat64Array()
var _spikes: Array = []                 # frames over SPIKE_MS after load, with render CPU/GPU times to attribute them
const SPIKE_MS := 8.3
const SPIKES_KEEP := 10
# Per-frame phase clocks (Godot's TIME_PROCESS monitor is the max of the last second, useless
# for one frame): this node processes first, _FrameEnd last, so their gap is all _process work.
var _process_t0 := 0
var _physics_t0 := 0
var _process_us := 0     # previous frame's _process phase
var _physics_us := 0     # physics steps since the previous frame (0, 1 or more)
var _physics_acc_us := 0
var _prev_mesh_b := 0.0      # RENDER_BUFFER_MEM_USED last frame: a spike's upload volume is the delta
var _prev_compiles := 0.0    # pipeline compilations so far (draw + specialization)
var _mesh_delta_mb := 0.0
var _compile_delta := 0.0
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
	inside = args.get("inside", "0") == "1"
	third_person = int(args.get("third_person", "0"))
	pitch = float(args.get("pitch", "-0.2"))
	walk = args.get("walk", "0") == "1"
	mine = args.get("mine", "0") == "1"
	place = args.get("place", "0") == "1"
	open_inventory = args.get("open_inventory", "0") == "1"
	scatter_items = args.get("scatter_items", "0") == "1"
	stations = args.get("stations", "0") == "1"
	open_screen = args.get("open", "")
	blast_m = float(args.get("blast", "0"))
	blast_count = int(args.get("blast_count", "1"))
	blast_every = float(args.get("blast_every", "2"))
	var defs := GameMaterials.defs()
	for i in defs.size():   # material ids are 1 + definition index (0 = air)
		if defs[i]["name"] == args.get("blast_material", "air"):
			blast_material = i + 1
	for spec in String(args.get("give", "")).split(",", false):
		var parts := spec.split(":")
		give.append({"id": parts[0], "count": int(parts[1]) if parts.size() > 1 else 1})
	if capture_every > 0.0:
		_strip = FrameSheet.new(320, 180, 5)
	if args.get("vsync", "1") == "0":
		DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	set_process(autopilot)
	if autopilot:
		RenderingServer.viewport_set_measure_render_time(get_viewport().get_viewport_rid(), true)
		process_priority = -1000000
		process_physics_priority = -1000000
		set_physics_process(true)
		var end := _FrameEnd.new()
		end.cap = self
		add_child(end)


## Runs after every other node's _process / _physics_process and closes the phase clocks.
class _FrameEnd extends Node:
	var cap: Node

	func _ready() -> void:
		process_priority = 1000000
		process_physics_priority = 1000000

	func _process(_d: float) -> void:
		cap._process_us = Time.get_ticks_usec() - cap._process_t0

	func _physics_process(_d: float) -> void:
		cap._physics_acc_us += Time.get_ticks_usec() - cap._physics_t0


func _physics_process(_delta: float) -> void:
	_physics_t0 = Time.get_ticks_usec()


func attach(main: Node) -> void:
	_main = main


func _process(delta: float) -> void:
	var prev_process_us := _process_us   # the frame whose delta we are about to record
	_physics_us = _physics_acc_us
	_physics_acc_us = 0
	_process_t0 = Time.get_ticks_usec()
	if _done or _main == null:
		return
	_elapsed += delta
	if _elapsed < 1.0:
		return  # let the world load
	if _start_pos == Vector3.ZERO:
		_start_pos = _main.player.position
	# wall clock between frame starts (this node processes first); `delta` is not it: the
	# physics jitter fix moves time between frames, so a 28 ms hitch can show as 20 ms
	var now := Time.get_ticks_usec()
	var ms := (now - _last_frame_us) / 1000.0 if _last_frame_us > 0 else delta * 1000.0
	var mesh_b := Performance.get_monitor(Performance.RENDER_BUFFER_MEM_USED)
	var compiles := Performance.get_monitor(Performance.PIPELINE_COMPILATIONS_DRAW) + Performance.get_monitor(Performance.PIPELINE_COMPILATIONS_SPECIALIZATION)
	_mesh_delta_mb = (mesh_b - _prev_mesh_b) / 1048576.0
	_compile_delta = compiles - _prev_compiles
	_prev_mesh_b = mesh_b
	_prev_compiles = compiles
	_last_frame_us = now
	if _skip_next:
		_skip_next = false
		_capture_stalls.push_back(ms)
	elif _loaded_at < 0.0:
		_load_max_ms = maxf(_load_max_ms, ms)
		if _world_settled() or _elapsed >= SETTLE_CAP_S:
			_loaded_at = _elapsed
	else:
		_frame_ms.push_back(ms)
		if ms > SPIKE_MS:
			_note_spike(ms, prev_process_us)
		if blast_m > 0.0:
			_blast_accum += delta
			if _blast_accum >= blast_every:
				_blast_accum -= blast_every
				_blast_volley()
	if _strip:
		_capture_accum += delta
		if _capture_accum >= capture_every:
			_capture_accum -= capture_every
			_strip.add(get_viewport())
			_skip_next = true
	if _loaded_at >= 0.0 and _elapsed >= _loaded_at + duration:   # `duration` seconds of measured, loaded frames
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
	s["max_frame_ms"] = snappedf(ft[ft.size() - 1] if ft.size() else 0.0, 0.01)
	s["frames_over_8_3"] = _count_over(ft, 8.3)
	s["frames_over_16_7"] = _count_over(ft, 16.7)
	s["load_s"] = snappedf(_loaded_at, 0.01)
	s["load_settled"] = _settled
	s["load_max_frame_ms"] = snappedf(_load_max_ms, 0.01)
	s["capture_stalls"] = _capture_stalls.size()
	s["capture_stall_max_ms"] = snappedf(Array(_capture_stalls).max() if _capture_stalls.size() else 0.0, 0.01)
	s["vram_mb"] = snappedf(Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0, 1)
	var host := _host_probe()
	s["rss_mb"] = host["rss_mb"]
	s["other_godot_processes"] = host["other_godot"]   # > 0: timings shared the machine with another run
	# ADR 0009 reference-machine budget, after load: p99 <= 8.3 ms, no frame > 16.7 ms, RAM <= 4 GB, VRAM <= 6 GB
	s["budget"] = {"p99": s["p99_frame_ms"] <= 8.3, "max": s["max_frame_ms"] <= 16.7,
		"ram": s["rss_mb"] >= 0.0 and s["rss_mb"] <= 4096.0, "vram": s["vram_mb"] <= 6144.0, "loaded": _loaded_at >= 0.0}
	# frames are counted only after load, so a run that never settled is judged on a
	# streaming world from SETTLE_CAP_S on: still a valid budget check, flagged in load_settled
	s["budget_ok"] = not s["budget"].values().has(false)
	s["spikes"] = _spikes
	if mine:
		s["mined_log"] = _main.player.mined_log
	if blast_m > 0.0:
		# the edits must have reached the meshed terrain (pushed from the bubble), not only the simulation
		_main.bubble.flush_all()
		var at := []
		for c in _blast_centres:
			at.append(GameMaterials.name_of(_main.world.get_voxel(c)))
		s["blast_terrain_at_centres"] = at
	s["settings"] = {"seed": Settings.world_seed, "seed_text": Settings.world_seed_text, "render_distance_m": Settings.render_distance_m, "view_distance_m": Settings.view_distance_m,
		"simulation_distance_m": Settings.simulation_distance_m, "lod_count": Settings.lod_count, "sim_tick_hz": Settings.sim_tick_hz}
	print(JSON.stringify(s))
	get_tree().quit(0)


## Where a slow frame went: delta is the time since the previous frame started, so it
## covers that frame's _process phase (all scripts and nodes incl. VoxelLodTerrain's
## main-thread update), the physics steps run since, and rendering. Render CPU/GPU
## times lag by a frame or two; `other_ms` is what none of these explain (driver,
## present, engine internals). Draw counts are of the last drawn frame.
func _note_spike(ms: float, process_us: int) -> void:
	var vp := get_viewport().get_viewport_rid()
	var rcpu := RenderingServer.viewport_get_measured_render_time_cpu(vp) + RenderingServer.get_frame_setup_time_cpu()
	var pms := process_us / 1000.0
	var phys := _physics_us / 1000.0
	_spikes.append({"frame": Engine.get_process_frames() - 1, "frame_ms": snappedf(ms, 0.01),
		"process_ms": snappedf(pms, 0.01), "physics_ms": snappedf(phys, 0.01),
		"render_cpu_ms": snappedf(rcpu, 0.01),
		"render_gpu_ms": snappedf(RenderingServer.viewport_get_measured_render_time_gpu(vp), 0.01),
		"other_ms": snappedf(maxf(ms - pms - phys - rcpu, 0.0), 0.01),
		"bubble": _main.bubble.last_frame_cost(),
		"draw_calls": Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		"primitives_m": snappedf(Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME) / 1e6, 0.01),
		"mesh_mb": snappedf(Performance.get_monitor(Performance.RENDER_BUFFER_MEM_USED) / 1048576.0, 1),
		"mesh_delta_mb": snappedf(_mesh_delta_mb, 0.01), "new_pipelines": _compile_delta,
		"voxel_tasks": VoxelEngine.get_stats().get("tasks", {})})
	_spikes.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a["frame_ms"] > b["frame_ms"])
	if _spikes.size() > SPIKES_KEEP:
		_spikes.resize(SPIKES_KEEP)


## One volley of blast_count craters on the ground, on a golden-angle spiral centred
## ahead of the camera (so the filmstrip sees them) and kept inside the simulation window.
func _blast_volley() -> void:
	var r := Settings.m2v(blast_m)
	var reach := maxf(Settings.sim_distance_chunks() * Settings.CHUNK - r - 2.0, 0.0)
	var p: Vector3 = _main.player.position
	var fwd: Vector3 = -_main.player.camera.global_transform.basis.z
	var ahead := Vector2(fwd.x, fwd.z).normalized() * reach * 0.5 if Vector2(fwd.x, fwd.z).length() > 0.01 else Vector2.ZERO
	for k in blast_count:
		var i := _blast_i
		_blast_i += 1
		var a := i * 2.39996
		var d := minf(r * 0.8 * sqrt(float(i % 64)), reach * 0.5)
		var x := int(p.x + ahead.x + cos(a) * d)
		var z := int(p.z + ahead.y + sin(a) * d)
		var c := Vector3i(x, _main.world.surface_height(x, z), z)
		if blast_material != 0:
			c.y += int(r) + 2   # a ball of it in the air above the ground, so it falls and spreads
		_main.bubble.blast(Vector3(c), r, blast_material)
		if _blast_centres.size() < 8:
			_blast_centres.append(c)


## "After load" (ADR 0009): the bubble holds every chunk and godot_voxel has had no
## generation, meshing or main-thread tasks for SETTLE_FRAMES frames in a row. The
## bubble alone loads in ~1 s, while the visible terrain is still meshing (frames with
## 0 primitives and 20-30 ms of main-thread mesh application).
func _world_settled() -> bool:
	var t: Dictionary = VoxelEngine.get_stats().get("tasks", {})
	var busy: int = int(t.get("generation", 0)) + int(t.get("meshing", 0)) + int(t.get("main_thread", 0)) + int(t.get("streaming", 0))
	_idle_frames = _idle_frames + 1 if busy == 0 and _main.bubble.is_loaded() else 0
	_settled = _idle_frames >= SETTLE_FRAMES
	return _settled


func _count_over(sorted_ms: PackedFloat64Array, limit: float) -> int:
	var i := sorted_ms.bsearch(limit, false)   # first index past every value <= limit
	return sorted_ms.size() - i


## Working set of this process (static_memory_usage misses the native extensions' heaps)
## and how many other Godot processes are alive: another session's benchmark on the same
## box shows up here as 20-30 ms frames that are not ours.
func _host_probe() -> Dictionary:
	var out := []
	var pid := OS.get_process_id()
	if OS.get_name() == "Windows":
		# two output lines: working set bytes, count of other Godot games/editors (each
		# console launch is a *_console wrapper plus the real process; count the real ones)
		OS.execute("powershell", ["-NoProfile", "-Command",
			"(Get-Process -Id %d).WorkingSet64; @(Get-Process -Name Godot* | Where-Object { $_.Id -ne %d -and $_.ProcessName -notlike '*_console' }).Count" % [pid, pid]], out)
	else:
		OS.execute("sh", ["-c", "ps -o rss= -p %d | awk '{print $1*1024}'; pgrep -c -i godot | awk '{print $1-1}'" % pid], out)
	var parts := String(out[0]).strip_edges().split("\n") if not out.is_empty() else PackedStringArray()
	if parts.size() < 2 or not parts[0].strip_edges().is_valid_int():
		return {"rss_mb": -1.0, "other_godot": -1}
	return {"rss_mb": snappedf(int(parts[0].strip_edges()) / 1048576.0, 0.1), "other_godot": int(parts[1].strip_edges())}