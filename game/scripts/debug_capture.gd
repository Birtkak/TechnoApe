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
var fly_after_cover := false   # fly_after_cover=1: the autopilot holds still until the start cover lifts, as a player does
var follow_ground := false   # fly at `alt` metres above the generated ground instead of a fixed height (low fast flight)
var target := ""      # lake|ruin|spring|river|waterfall|marsh|alpine|snow|mountain|cave|<biome name>: start before the nearest one and fly over it
var offset_m := 40.0  # start this far from the target (along +z)
var inside := false   # start at the target position itself (cave|...) with a debug torch, instead of above it
var third_person := 0 # 1 = camera behind the ape, 2 = in front (F5 views)
var pitch := -0.2     # camera pitch in radians (negative = looking down)
var yaw := 0.0        # camera yaw in radians (0 = the default heading; sky shots, ADR 0025)
var walk := false     # walk (gravity, step-up) instead of flying; reports distance travelled
var mine := false     # stand still, look down and hold break for the whole run; JSON reports picked_up
var place := false    # stand still and place the held item ahead every place_repeat_s; JSON shows the hotbar count drop
var open_inventory := false   # open the inventory screen (for HUD screenshots)
var scatter_items := false    # also drop one item entity per given item in front of the player
var stations := false         # place a crafting table, chest and furnace in front of the player; the furnace gets iron ore + coal
var open_screen := ""          # open=crafting_table|chest|furnace: open that station's screen (needs stations=1)
var give: Array = []  # give=oak_log:12,wooden_pickaxe  -> starting items
var chat_commands: PackedStringArray = []  # commands=/gamemode creative;/time set noon  -> run through the chat at start (ADR 0018)
var chat_commands_after: PackedStringArray = []  # commands_after=/tp 10 1030 20;wait 3;/blast 2 lava  -> run once the world has loaded ("wait N" pauses N s)
var chat_text := ""   # chat_text=/give oak  -> chat box open with this typed (screenshot of hints)
var debug_menu := false   # debug_menu=1: open the F4 playtest menu
var f3 := false       # f3=1: the F3 overlay on
var blast_m := 0.0    # blast=R: mass-edit stress (ADR 0012): craters of radius R m on the ground around the player
var blast_count := 1  # blast_count=N craters per volley (1000 TNT = many small ones)
var blast_every := 2.0  # blast_every=S seconds between volleys, first one 2 s after load
var blast_material := 0  # blast_material=water|sand|...: fill instead of carve (liquid spills: remesh stress)
var animals := 0         # animals=N: N sheep and pigs 4-10 m ahead once loaded (ADR 0022); animal=sheep|pig only that one
var animal_species := ""
var animal_babies := false   # animal_babies=1: every third one a baby
var animal_love := false     # animal_love=1: adults start in love (they pair up and breed)
var farm_test := false   # farm_test=1: a sown 9x9 field of 0.5 m blocks around a block of water ahead (ADR 0022)
var farm_speed := 1.0    # farm_speed=N: farmland and crop random ticks N times faster
var swim_test := false  # swim_test=1: sink to a pool floor, lose air, drown, hop out onto the bank (JSON line "debug swim")
var hands_test := false # hands_test=1: till, sow, chair, bed, feed, shear, kill, eat through the player's input code (JSON line "debug hands")
var blueprint_test := ""   # blueprint_test=1: copy a hut with a furnace, rebuild it by a builder (ADR 0161); JSON "debug blueprint", sheet blueprint.png;
                           # =save then =load (persist=1 seed=<unused>): a held written blueprint and a marked site survive a reload
var building_test := false # building_test=1: place every ADR 0094 piece in a room, open the door, climb; JSON "debug building", sheet building.png
var menu_test := false   # menu_test=1: title, worlds, Esc menu, settings, save and reload a world (ADR 0032, JSON line "debug menu_test")
var _menu_test: MenuTest
var flint_age_test := false # flint_age_test=1: hides, charcoal pit, pan, spear, torches, shelter, bed, pot, bag, stew (ADR 0030, JSON line "debug flint_age")
var later: PackedStringArray = []   # later=/a;wait 1.5;/b: chat commands after loading, in order ("wait <s>" pauses)
var chop_test := ""         # chop_test=1[,cam]: 1.5 s after loading, /chop [cam] (P12 rigid pieces)
var fell_test := ""         # fell_test=<deg>[,cam]: 1.5 s after loading, /fell <deg> [cam] (P12 rigid pieces)
var water_test := false     # water_test=1 (with target=river): dig a trench into the bank, check it fills to the river's level (ADR 0045, JSON line "debug water")
var showcase := ""         # showcase=flint_stones,pebbles [showcase_on=grass]: those side by side on a pad, one sheet, then quit (Showcase)
var showcase_on := "grass"
var looks_test := false     # looks_test=1: part C's multi-voxel crops, flowers, torches, table and lights on a patch, shot (ADR 0053, "debug looks")
var looks_shots := true     # looks_shots=0: looks_test without screenshots
var looks_budget := false   # looks_budget=1: looks_test only sets ten torches and a held one at night (the budget case)
var ponder_only := ""        # ponder_only=<scene id>: ponder_test films just that one
var ponder_test := false    # ponder_test=1: every guide scene filmed, a row a step, to out/ponder/<id>.png ("debug ponder", ADR 0132)
var holes := false          # holes=1: the ADR 0136 census of ground in view that is not drawn, by pipeline stage (JSON "holes"; costs frame time)
var shell_test := false     # shell_test=1: cave + dug hole, counts unloaded / unmeshed blocks and air-where-rock within 16 m (ADR 0124, scripts/debug/shell_test.gd, "debug shell")
var fluid_lag := ""         # fluid_lag=lake|river|bomb|pour: time a fluid change to its remesh and rest (ADR 0137, "debug fluid_lag")
var fluid_lag_sheet := false
var fluid_lag_from := ""
var proc_probe := false   # proc_probe=1: each _process phase over 12 ms names its slowest subtrees (ProcProbe)
var mine_test := ""         # mine_test=collapse|propped|pick|laser|save|reload: dug roofs creak and fall or hold (ADR 0142, scripts/debug/mine_test.gd, "debug mine")
var mine_rate := 4.0        # mine_rate=N: 0.5 m blocks a second in mine_test=pick
var sand_test := false      # sand_test=1: a ball of sand dropped onto still ocean water must sink (scripts/debug/sand_test.gd, "debug sand")
var toys_test := false      # toys_test=1: the glider, climbing rope and grappling hook, scripted and filmed (scripts/debug/toys_test.gd, "debug toys")
var tp_test := false        # tp_test=1: a burst of teleports (hops, far jumps, /spawn, /goto, /kill), timed and filmed (scripts/debug/tp_test.gd, "debug tp")
var fluid_film := false     # fluid_film=1: water scenes filmed as strips + bursts + sim slices (scripts/visuals/fluid_film.gd, ADR 0138)
var liquids_test := false   # liquids_test=1: a pool of every liquid side by side, shot by day and night, and items dropped into lava (bug reports 2026-10-25, "debug liquids")
var water_shots := true     # water_shots=0: no screenshots in the water test
var stone_ages_test := false # stone_ages_test=1: the four Stone Age stages alone (flint_age_test=1 runs them too; ADR 0059, JSON line "debug stone_ages")
var craft_test := false     # craft_test=1: assemble an axe, work a firepit by hand, smith a hammer head, wear a tool out (ADR 0042, JSON line "debug craft")
var info_test := false      # info_test=1: the look-at panel on ore and a loaded forge, R / U, station tabs, bookmarks: info.png (ADR 0093, "debug info")
var metal_age_test := false # metal_age_test=1: bronze in a firepit, a bloomery's bloom, the forge, the bronze anvil (ADR 0070, JSON line "debug metal")
var stone_age_test := false # stone_age_test=1: knap, form clay, pit kiln, firepit, cast, sieve through the player's clicks (ADR 0023, JSON line "debug stone_age")
var mob_spawning := true # mob_spawn=0: no herds spawn as land loads (benchmarks without animals)
var mob_bench := 0       # mob_bench=N: N animals (animal=, default bear) within 6 m of the player standing on the ground once loaded; frames count from 2 s later (bug report 2026-10-25 21-11-51: 100 bears, 4 fps)
var mob_bench_r := 6.0  # mob_bench_r=M: scatter radius in metres around the spot 8 m ahead
var _bench_wait := -1.0
var stream_probe := false   # stream_probe=1: every 1 s the voxel queues, worker time per stage and the holes in the box (ADR 0135, JSON "stream")
var _sp_offsets: Array[Vector2] = []   # patch centres (voxels, from the player) on an 8 m grid out to the render distance
var _sp_i := 0
var _sp_holes := {}        # band (m) -> patches not drawn, this sweep
var _sp_last := {}         # the last finished sweep
var _sp_accum := 0.0
var _sp_prev := {}
var _sp_series: Array = []
var _sp_ms_max := 0.0
var _probe_ms_max := 0.0   # the detail probe's own cost (is_area_meshed), JSON detail.probe_ms_max
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
var settle_cap_s := SETTLE_CAP_S   # load_cap_s= raises it to time a slow first load (2 km horizons)
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
var proc_cost := {}      # ms per node's _process this frame (note_proc); the spike record shows the previous frame's
var _proc_prev := {}
var _physics_us := 0     # physics steps since the previous frame (0, 1 or more)
var _physics_acc_us := 0
var _prev_mesh_b := 0.0      # RENDER_BUFFER_MEM_USED last frame: a spike's upload volume is the delta
var _prev_compiles := 0.0    # pipeline compilations so far (draw + specialization)
var _mesh_delta_mb := 0.0
var _others_max := 0         # most other Godot processes seen by the periodic probe
var _others_task := -1
var _others_accum := 0.0
const OTHERS_EVERY_S := 10.0
var _backlog := PackedInt32Array()   # pending voxel tasks (generation + meshing + streaming), sampled every 0.25 s after load
var _backlog_accum := 0.0
var _backlog_peak := {}              # per task type (generation, meshing, streaming, main_thread, gpu)
var _detail_missing := PackedInt32Array()   # 1 = the ground 18 m around the player had no mesh yet, sampled with the backlog
# ADR 0136: ground mesh blocks in the camera's view inside the solid part of the box that are not drawn, by the
# pipeline stage they wait in (VoxelLodTerrain.debug_classify_mesh_blocks, voxel_patch_holes.py), sampled with the backlog
const HOLE_KINDS := ["drawn", "hidden", "not_requested", "generating", "mesh_pending", "meshing", "upload_wait", "meshed_empty", "not_in_box"]
var _holes := {}               # kind -> block-samples
var _holes_by_band := {}       # "0" / "16" / "32" m from the camera -> block-samples not drawn
var _holes_visible := {}       # holes the camera has a clear line to over the true ground (FineGen heights): kind -> count
var _holes_visible_band := {}  # the same by distance band
var _holes_pending := {}       # mesh_pending blocks in view: key -> usec first seen pending (how long data-complete blocks wait)
var _holes_pending_state := {} # their mesh state (VoxelLodTerrainUpdateData::MeshState) -> count
var _holes_pending_ms: Array[float] = []
var _holes_gap := 0            # block-samples drawn at the previous sample and not now (a dropped mesh)
var _holes_series := PackedInt32Array()   # holes per sample
var _holes_seen := 0           # ground blocks in view, summed over samples
var _holes_prev := {}          # blocks drawn at the previous sample
var _holes_ms_max := 0.0
var _holes_empty_checked := 0
var _holes_empty_examples := []   # meshed-empty ground blocks that hold more than one material (a mesher hole?)
var _compile_delta := 0.0
var _elapsed := 0.0
var _capture_accum := 0.0
var full_frames := false   # full_frames=1: every filmstrip capture also saved full size as frame_NNN.png (particles, small detail)
var _full_i := 0
var _main: Node
var _done := false


## The title screen calls this once it is up (menu_test=1 drives the menus from here,
## because the scenes it walks through come and go).
func menu_test_ready(title: Control) -> void:
	if menu_test and _menu_test == null:
		_menu_test = MenuTest.new()
		_menu_test.run(self, title)


func _ready() -> void:
	var args: Dictionary = Settings.apply_cmdline()
	autopilot = args.get("autopilot", "0") == "1"
	if autopilot and not args.has("persist"):
		if not args.has("persist"):   # persist=1 seed=<unused seed>: measure the save-file (SQLite stream) load path
			Settings.persist_world = false  # unattended test runs never write world saves
	duration = float(args.get("duration", "20"))
	settle_cap_s = float(args.get("load_cap_s", str(SETTLE_CAP_S)))
	if autopilot:
		_start_watchdog(float(args.get("watchdog_s", str(settle_cap_s + duration + 60.0))))
		if args.get("show", "0") != "1":   # unattended runs stay off the developer's screen; show=1 keeps it
			DisplayServer.window_set_position(Vector2i(-30000, -30000))
	capture_every = float(args.get("capture_every", "0"))
	full_frames = args.get("full_frames", "0") == "1"
	out_dir = args.get("out", "res://out")
	altitude_m = float(args.get("alt", "30"))
	speed_m = float(args.get("speed", "6"))
	fly_after_cover = args.get("fly_after_cover", "0") == "1"
	follow_ground = args.get("follow_ground", "0") == "1"
	target = args.get("target", "")
	offset_m = float(args.get("offset", "40"))
	inside = args.get("inside", "0") == "1"
	third_person = int(args.get("third_person", "0"))
	pitch = float(args.get("pitch", "-0.2"))
	yaw = float(args.get("yaw", "0.0"))
	walk = args.get("walk", "0") == "1"
	mine = args.get("mine", "0") == "1"
	place = args.get("place", "0") == "1"
	open_inventory = args.get("open_inventory", "0") == "1"
	scatter_items = args.get("scatter_items", "0") == "1"
	stations = args.get("stations", "0") == "1"
	open_screen = args.get("open", "")
	chat_commands = String(args.get("commands", "")).split(";", false)
	chat_commands_after = String(args.get("commands_after", "")).split(";", false)
	chat_text = args.get("chat_text", "")
	debug_menu = args.get("debug_menu", "0") == "1"
	f3 = args.get("f3", "0") == "1"
	blast_m = float(args.get("blast", "0"))
	blast_count = int(args.get("blast_count", "1"))
	blast_every = float(args.get("blast_every", "2"))
	animals = int(args.get("animals", "0"))
	mob_bench = int(args.get("mob_bench", "0"))
	mob_bench_r = float(args.get("mob_bench_r", "6"))
	animal_species = args.get("animal", "")
	animal_babies = args.get("animal_babies", "0") == "1"
	animal_love = args.get("animal_love", "0") == "1"
	farm_test = args.get("farm_test", "0") == "1"
	farm_speed = float(args.get("farm_speed", "1"))
	mob_spawning = args.get("mob_spawn", "1") == "1"
	hands_test = args.get("hands_test", "0") == "1"
	building_test = args.get("building_test", "0") == "1"
	blueprint_test = String(args.get("blueprint_test", "0")).replace("0", "")
	swim_test = args.get("swim_test", "0") == "1"
	fell_test = args.get("fell_test", "")
	chop_test = args.get("chop_test", "")
	later = String(args.get("later", "")).split(";", false)
	stone_age_test = args.get("stone_age_test", "0") == "1"
	flint_age_test = args.get("flint_age_test", "0") == "1"
	craft_test = args.get("craft_test", "0") == "1"
	metal_age_test = args.get("metal_age_test", "0") == "1"
	info_test = args.get("info_test", "0") == "1"
	stone_ages_test = args.get("stone_ages_test", "0") == "1"
	water_test = args.get("water_test", "0") == "1"
	showcase = args.get("showcase", "")
	showcase_on = args.get("showcase_on", "grass")
	looks_test = args.get("looks_test", "0") == "1"
	looks_shots = args.get("looks_shots", "1") != "0"
	looks_budget = args.get("looks_budget", "0") == "1"
	liquids_test = args.get("liquids_test", "0") == "1"
	fluid_film = args.get("fluid_film", "0") == "1"
	tp_test = args.get("tp_test", "0") == "1"
	toys_test = args.get("toys_test", "0") == "1"
	ponder_test = args.get("ponder_test", "0") == "1"
	ponder_only = String(args.get("ponder_only", ""))
	shell_test = args.get("shell_test", "0") == "1"
	holes = args.get("holes", "0") == "1"
	sand_test = args.get("sand_test", "0") == "1"
	mine_test = args.get("mine_test", "")
	mine_rate = float(args.get("mine_rate", "4"))
	fluid_lag = args.get("fluid_lag", "")
	fluid_lag_sheet = args.get("fluid_lag_sheet", "0") == "1"
	proc_probe = args.get("proc_probe", "0") == "1"
	fluid_lag_from = args.get("fluid_lag_from", "")
	water_shots = args.get("water_shots", "1") != "0"
	menu_test = args.get("menu_test", "0") == "1"
	stream_probe = args.get("stream_probe", "0") == "1"
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
		Engine.max_fps = int(args.get("max_fps", "0"))   # benchmarks run uncapped (the project caps play at 120 fps)
	set_process(autopilot)
	if autopilot:
		RenderingServer.viewport_set_measure_render_time(get_viewport().get_viewport_rid(), true)
		process_priority = -1000000
		process_physics_priority = -1000000
		set_physics_process(true)
		var end := _FrameEnd.new()
		end.cap = self
		add_child(end)


## A node's _process cost this frame (ms since t0), for the spike records ("proc_cost").
func note_proc(name: String, t0: int) -> void:
	proc_cost[name] = maxf(float(proc_cost.get(name, 0.0)), (Time.get_ticks_usec() - t0) / 1000.0)


## Runs after every other node's _process / _physics_process and closes the phase clocks.
class _FrameEnd extends Node:
	var cap: Node

	func _ready() -> void:
		process_priority = 1000000
		process_physics_priority = 1000000

	func _process(_d: float) -> void:
		cap._process_us = Time.get_ticks_usec() - cap._process_t0
		cap._proc_prev = cap.proc_cost
		cap.proc_cost = {}

	func _physics_process(_d: float) -> void:
		cap._physics_acc_us += Time.get_ticks_usec() - cap._physics_t0


func _physics_process(_delta: float) -> void:
	_physics_t0 = Time.get_ticks_usec()
	if follow_ground and _main != null and _main.player != null and _loaded_at >= 0.0:
		# a ghost flight: moved here along the heading (through trees and hills), `alt` above the ground
		var pl: Node3D = _main.player
		pl.autopilot_speed_m = 0.0
		var fwd := -pl.transform.basis.z
		fwd.y = 0.0
		var pp: Vector3 = pl.position + fwd.normalized() * Settings.m2v(speed_m) * _delta
		pp.y = float(_main.world.generator.gen.height_at(int(pp.x), int(pp.z))) + Settings.m2v(altitude_m)
		pl.position = pp
	if _main != null and _main.phys_cost.size() > 0 and _physics_acc_us == 0:
		_main.phys_cost.clear()   # first step of this frame: start the per-callback maxima afresh


func attach(main: Node) -> void:
	_main = main


var _parts := {"render_cpu": [], "render_gpu": [], "process": [], "physics": []}
var _parts_n := 0


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
		if _bench_wait > 0.0:
			_bench_wait -= delta
			if _bench_wait <= 0.0:
				_loaded_at = _elapsed
		elif _world_settled() or _elapsed >= settle_cap_s:
			if mob_bench > 0 and _bench_wait < 0.0:
				_spawn_mob_bench()
				_bench_wait = 2.0   # the models build and the clump settles; then the measured frames start
			else:
				_loaded_at = _elapsed
				_print_probe_voxels()
	else:
		_frame_ms.push_back(ms)
		# where a normal frame goes: script and physics time every frame; the render
		# CPU / GPU times every 30th frame only, because reading them waits for the
		# render thread (the frame after a read is left out, like a filmstrip capture)
		_parts["process"].push_back(prev_process_us / 1000.0)
		_parts["physics"].push_back(_physics_us / 1000.0)
		_parts_n += 1
		if _parts_n % 30 == 0 and not _strip:
			var vp := get_viewport().get_viewport_rid()
			_parts["render_cpu"].push_back(RenderingServer.viewport_get_measured_render_time_cpu(vp) + RenderingServer.get_frame_setup_time_cpu())
			_parts["render_gpu"].push_back(RenderingServer.viewport_get_measured_render_time_gpu(vp))
			_skip_next = true
		if ms > SPIKE_MS:
			_note_spike(ms, prev_process_us)
			_skip_next = true   # the note waits for the render thread: the next frame pays it, as after a capture
		if blast_m > 0.0:
			_blast_accum += delta
			if _blast_accum >= blast_every:
				_blast_accum -= blast_every
				_blast_volley()
	# another Godot starting mid-run (a peer's benchmark) must taint the run even if it
	# exits before ours ends; the probe runs on a worker thread, never on the frame
	_backlog_accum += delta
	if _loaded_at >= 0.0 and _backlog_accum >= 0.25:
		_backlog_accum = 0.0
		var t: Dictionary = VoxelEngine.get_stats().get("tasks", {})
		_backlog.push_back(int(t.get("generation", 0)) + int(t.get("meshing", 0)) + int(t.get("streaming", 0)) + int(t.get("main_thread", 0)))
		for k in t:   # per-type peak, to tell a generator backlog from a meshing one
			_backlog_peak[k] = maxi(int(_backlog_peak.get(k, 0)), int(t[k]))
		# what a flying player sees: is the ground around them drawn yet (bug report 2026-10-25)
		var pp: Vector3 = _main.player.position
		# a 4 m patch every 4 m out to 18 m, each from 3 m under its own ground to 1 m over it: a flat box cut
		# into hillsides deeper than the ground shell (ADR 0105) loads, and buried blocks are never drawn.
		var step := Settings.m2v(4.0)
		var under := Settings.m2v(3.0)
		var tp := Time.get_ticks_usec()
		var drawn := true
		for ix in range(-4, 5):
			for iz in range(-4, 5):
				var x := pp.x + ix * step
				var z := pp.z + iz * step
				var h := float(_main.world.generator.gen.height_at(int(x), int(z)))
				var patch := AABB(Vector3(x - step * 0.5, h - under, z - step * 0.5), Vector3(step, under + Settings.m2v(1.0), step))
				if not _patch_drawn(patch):
					drawn = false
					break
			if not drawn:
				break
		_detail_missing.push_back(0 if drawn else 1)
		_probe_ms_max = maxf(_probe_ms_max, (Time.get_ticks_usec() - tp) / 1000.0)
		if holes:
			_holes_sample()
	if stream_probe and _loaded_at >= 0.0:
		_stream_probe_step(delta)
	_others_accum += delta
	if _others_accum >= OTHERS_EVERY_S and (_others_task < 0 or WorkerThreadPool.is_task_completed(_others_task)):
		if _others_task >= 0:
			WorkerThreadPool.wait_for_task_completion(_others_task)
		_others_accum = 0.0
		_others_task = WorkerThreadPool.add_task(_probe_others)
	if _strip:
		_capture_accum += delta
		if _capture_accum >= capture_every:
			_capture_accum -= capture_every
			_strip.add(get_viewport())
			if full_frames and _loaded_at >= 0.0:
				if _full_i == 0:
					DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(out_dir))
				get_viewport().get_texture().get_image().save_png(out_dir + "/frame_%03d.png" % _full_i)
				_full_i += 1
			_skip_next = true
	if _loaded_at >= 0.0 and _elapsed >= _loaded_at + duration:   # `duration` seconds of measured, loaded frames
		_finish()


## mob_bench=N: the player stands on the ground looking at N animals scattered within mob_bench_r (6) m of a
## spot 8 m ahead (a /summon clump, the bug report's case); the JSON "mobs" block holds their cost.
func _spawn_mob_bench() -> void:
	var pl: Node3D = _main.player
	pl.autopilot_speed_m = 0.0
	pl.flying = false
	var gh: int = _main.world.surface_height(int(floor(pl.position.x)), int(floor(pl.position.z)))
	pl.position.y = gh + 2
	var fwd: Vector3 = -pl.camera.global_transform.basis.z
	fwd.y = 0.0
	fwd = fwd.normalized() if fwd.length() > 0.01 else Vector3.FORWARD
	var centre: Vector3 = pl.position + fwd * Settings.m2v(8.0)
	var rng := RandomNumberGenerator.new()
	rng.seed = 1234
	var sp: String = animal_species if animal_species != "" else "bear"
	for i in mob_bench:
		var q := centre + Vector3(rng.randf_range(-1, 1), 0, rng.randf_range(-1, 1)) * Settings.m2v(mob_bench_r)
		var y: int = _main.world.surface_height(int(floor(q.x)), int(floor(q.z))) + 1
		_main.mobs.spawn(sp, Vector3(q.x, y + 0.05, q.z), false, rng.randf() * TAU)
	pl.set_look(atan2(-fwd.x, -fwd.z), -0.35)
	print("mob_bench: %d %s spawned" % [mob_bench, sp])


func _finish() -> void:
	_done = true
	var ft := _frame_ms.duplicate()
	ft.sort()
	var n := maxi(1, ft.size())
	var sum := 0.0
	for v in ft:
		sum += v
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(out_dir))
	var final_img := get_viewport().get_texture().get_image()
	if final_img:   # null with --headless: the JSON line still comes
		final_img.save_png(out_dir + "/final.png")
	if _strip and _strip.count() > 0:
		_strip.save(out_dir + "/strip.png")
	var s: Dictionary = _main.snapshot()
	s["test"] = "autopilot"
	s["mode_requested"] = "walk" if walk else "fly"
	s["travelled_m"] = snappedf(Settings.v2m((_main.player.position - _start_pos).length()), 0.1)
	s["travelled_horizontal_m"] = snappedf(Settings.v2m(Vector2(_main.player.position.x - _start_pos.x, _main.player.position.z - _start_pos.z).length()), 0.1)
	if _main.has_method("verify_bubble"):
		s["bubble_verify"] = _main.verify_bubble()
	s["near_unloaded"] = _near_unloaded()
	if stream_probe:
		s["stream"] = {"series": _sp_series, "probe_ms_max": snappedf(_sp_ms_max, 0.01)}
	s["frames"] = ft.size()
	s["avg_frame_ms"] = snappedf(sum / n, 0.01)
	s["p50_frame_ms"] = snappedf(ft[n / 2] if ft.size() else 0.0, 0.01)
	s["p95_frame_ms"] = snappedf(ft[int(n * 0.95)] if ft.size() else 0.0, 0.01)
	s["p99_frame_ms"] = snappedf(ft[int(n * 0.99)] if ft.size() else 0.0, 0.01)
	s["max_frame_ms"] = snappedf(ft[ft.size() - 1] if ft.size() else 0.0, 0.01)
	var parts := {}
	for k in _parts:
		var a: Array = _parts[k].duplicate()
		a.sort()
		if not a.is_empty():
			parts[k] = {"p50": snappedf(a[a.size() / 2], 0.01), "p99": snappedf(a[int(a.size() * 0.99)], 0.01)}
	s["frame_parts"] = parts   # steady state: render_cpu, render_gpu, process (scripts), physics, ms
	s["frames_over_8_3"] = _count_over(ft, 8.3)
	s["frames_over_16_7"] = _count_over(ft, 16.7)
	s["load_s"] = snappedf(_loaded_at, 0.01)
	s["load_settled"] = _settled
	s["load_max_frame_ms"] = snappedf(_load_max_ms, 0.01)
	s["capture_stalls"] = _capture_stalls.size()
	s["capture_stall_max_ms"] = snappedf(Array(_capture_stalls).max() if _capture_stalls.size() else 0.0, 0.01)
	s["vram_mb"] = snappedf(Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0, 1)
	if _main.world.terrain.has_method("get_ao_pages_used"):   # ADR 0107: occlusion page layers in use
		s["ao_pages_used"] = _main.world.terrain.get_ao_pages_used()
	var host := _host_probe()
	s["rss_mb"] = host["rss_mb"]
	if _others_task >= 0:
		WorkerThreadPool.wait_for_task_completion(_others_task)
	s["other_godot_processes"] = maxi(host["other_godot"], _others_max)   # > 0 at any probe: timings shared the machine
	# ADR 0009 reference-machine budget, after load: p99 <= 8.3 ms, no frame > 16.7 ms, RAM <= 8 GB (raised from 4 GB, developer 2026-10-27), VRAM <= 6 GB
	s["budget"] = {"p99": s["p99_frame_ms"] <= 8.3, "max": s["max_frame_ms"] <= 16.7,
		"ram": s["rss_mb"] >= 0.0 and s["rss_mb"] <= 8192.0, "vram": s["vram_mb"] <= 6144.0, "loaded": _loaded_at >= 0.0}
	# frames are counted only after load, so a run that never settled is judged on a
	# streaming world from SETTLE_CAP_S on: still a valid budget check, flagged in load_settled
	s["budget_ok"] = not s["budget"].values().has(false)
	s["spikes"] = _spikes
	# terrain streaming backlog while moving: how far godot_voxel's workers fall behind
	var bl := Array(_backlog)
	s["voxel_backlog"] = {"samples": bl.size(), "peak_by_type": _backlog_peak, "last": VoxelEngine.get_stats().get("tasks", {}), "max": bl.max() if bl.size() else 0,
		"mean": snappedf(float(bl.reduce(func(a, b): return a + b, 0)) / maxi(bl.size(), 1), 0.1),
		"busy_share": snappedf(float(bl.filter(func(v): return v > 0).size()) / maxi(bl.size(), 1), 0.01),
		"max_busy_s": _longest_run(bl) * 0.25,
		"threads": VoxelEngine.get_thread_count()}
	# what generation spent its worker time on over the run (ADR 0105): FineGen by result, godot_voxel per task
	var ts: Dictionary = _main.world.terrain.get_statistics() if _main != null and _main.world != null else {}
	s["generation"] = {"finegen": FineGen.gen_stats(), "task_us": ts.get("ta_gen_run_sum", 0), "tasks": ts.get("ta_gen_run_n", 0)}
	# the update thread's holds of the loading-blocks lock the main thread's data applies wait on (ADR 0136, run maxima)
	s["loading_lock"] = {"hold_max_us": ts.get("ta_loading_hold_max", -1), "holds_over_1ms": ts.get("ta_loading_hold_slow_n", -1), "rehashes": ts.get("ta_loading_rehash_n", -1)}
	var dm := Array(_detail_missing)
	s["detail"] = {"samples": dm.size(), "missing_share": snappedf(float(dm.count(1)) / maxi(dm.size(), 1), 0.01),
		"max_missing_s": _longest_run(dm) * 0.25, "probe_ms_max": snappedf(_probe_ms_max, 0.01),
		"series": "".join(dm.map(func(v: int) -> String: return str(v)))}   # one char per 0.25 s, 1 = missing
	var hs := Array(_holes_series)
	s["holes"] = {"samples": hs.size(), "seen": _holes_seen, "by_kind": _holes, "by_band_m": _holes_by_band, "visible": _holes_visible, "visible_by_band_m": _holes_visible_band, "pending_state": _holes_pending_state, "pending_wait_ms": _pct(_holes_pending_ms), "gap": _holes_gap, "empty_examples": _holes_empty_examples,
		"total": hs.reduce(func(a, b): return a + b, 0), "max": hs.max() if hs.size() else 0,
		"samples_with_holes": hs.filter(func(v): return v > 0).size(), "ms_max": snappedf(_holes_ms_max, 0.01),
		"series": ",".join(hs.map(func(v: int) -> String: return str(v)))}
	if _main.world.terrain.has_method("debug_request_counts"):   # lod 0 data requests / those waiting as buried, and generated of each (ADR 0136)
		var rc: Vector4i = _main.world.terrain.debug_request_counts()
		s["holes"]["requests"] = {"lod0": rc.x, "buried": rc.y, "generated": rc.z, "generated_buried": rc.w}
	if mine:
		s["mined_log"] = _main.player.mined_log
	if blast_m > 0.0:
		# the edits must have reached the meshed terrain (pushed from the bubble), not only the simulation
		_main.bubble.flush_all()
		var at := []
		for c in _blast_centres:
			at.append(GameMaterials.name_of(_main.world.get_voxel(c)))
		s["blast_terrain_at_centres"] = at
	s["startup_ms"] = _main.startup_ms
	if Net.active:   # ADR 0145: ops, bandwidth, where the other players are drawn, where this one stands (rejoin check)
		s["net"] = Net.snapshot()
		var pp: Vector3 = _main.player.position
		s["net"]["pos"] = [snappedf(pp.x, 0.1), snappedf(pp.y, 0.1), snappedf(pp.z, 0.1)]
		if _main.remote_players:
			s["net"]["avatars"] = _main.remote_players.positions()
			s["net"]["looks"] = _main.remote_players.looks()
			s["net"]["map"] = _main.remote_players.others().keys()   # the names the map and minimap mark
			s["net"]["my_colour"] = Net.my_colour
	s["settings"] = {"seed": Settings.world_seed, "seed_text": Settings.world_seed_text, "view_distance_m": Settings.view_distance_m,
		"simulation_distance_m": Settings.simulation_distance_m, "sim_tick_hz": Settings.sim_tick_hz}
	print(JSON.stringify(s))
	if _main.has_method("save_terrain"):
		_main.save_terrain(5.0)   # persist=1 runs keep their edits, as a save-and-quit does
	get_tree().quit(0)


## An autopilot run that hangs (loading, or quitting) kills itself instead of leaving a grey window:
## a thread kills the process limit_s after start. It touches only the shared `_deadline` array
## ([deadline_ms, stop]), never this node; a normal quit stops and joins it.
var _watchdog: Thread
var _deadline := [0, false]


func _start_watchdog(limit_s: float) -> void:
	_deadline[0] = Time.get_ticks_msec() + int(limit_s * 1000.0)
	var deadline := _deadline
	_watchdog = Thread.new()
	_watchdog.start(func() -> void:
		while not deadline[1] and Time.get_ticks_msec() < int(deadline[0]):
			OS.delay_msec(250)
		if not deadline[1]:
			printerr("autopilot watchdog: the run did not finish in time, killing it")
			OS.kill(OS.get_process_id()))


func _exit_tree() -> void:
	if _watchdog != null:
		_deadline[1] = true
		_watchdog.wait_to_finish()


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
	_spikes.append({"frame": Engine.get_process_frames() - 1, "frame_ms": snappedf(ms, 0.01), "t": snappedf(_elapsed - _loaded_at, 0.01),
		"process_ms": snappedf(pms, 0.01), "physics_ms": snappedf(phys, 0.01),
		"render_cpu_ms": snappedf(rcpu, 0.01),
		"render_gpu_ms": snappedf(RenderingServer.viewport_get_measured_render_time_gpu(vp), 0.01),
		"other_ms": snappedf(maxf(ms - pms - phys - rcpu, 0.0), 0.01),
		"bubble": _main.bubble.last_frame_cost(),
		"phys_cost": _main.phys_cost.duplicate(),
		"proc_cost": _proc_prev.duplicate(),
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
## probe_voxels=x,y,z;x,y,z: what the world holds there once loaded (a save round trip: edit, leave, reload).

## ADR 0136: every mesh block holding ground or water surface (FineGen.surface_map, a sample every 4 m) inside the
## solid part of the box (before the horizon fade) and the camera frustum, sorted by why it is not drawn.
func _holes_sample() -> void:
	var terrain = _main.world.terrain
	if not terrain.has_method("debug_classify_mesh_blocks"):
		return
	var t0 := Time.get_ticks_usec()
	var cam: Camera3D = get_viewport().get_camera_3d()
	var cp := cam.global_position
	var planes := cam.get_frustum()
	var s: int = terrain.mesh_block_size
	var solid := Settings.m2v(Settings.view_distance_m * Settings.horizon_fade_frac)
	var b0 := Vector2i(floori((cp.x - solid) / s), floori((cp.z - solid) / s))
	var cols := ceili(2.0 * solid / s) + 1
	var half := s / 2
	var n := cols * 2 + 1
	var sm: PackedInt32Array = _main.world.generator.gen.surface_map(b0.x * s, b0.y * s, n, half)
	var xyz := PackedInt32Array()
	var bands := PackedInt32Array()
	var seen := PackedByteArray()
	for j in cols:
		for i in cols:
			var lo := 1 << 30
			var hi := -(1 << 30)
			for dj in 3:
				for di in 3:
					var k := ((j * 2 + dj) * n + i * 2 + di) * 3
					lo = mini(lo, sm[k])
					hi = maxi(hi, maxi(sm[k], sm[k + 1]))
			var x0 := (b0.x + i) * s
			var z0 := (b0.y + j) * s
			var dx := maxf(maxf(x0 - cp.x, cp.x - (x0 + s)), 0.0)
			var dz := maxf(maxf(z0 - cp.z, cp.z - (z0 + s)), 0.0)
			if dx * dx + dz * dz > solid * solid:
				continue
			for by in range(floori((lo - 1.0) / s), floori(float(hi) / s) + 1):
				var box := AABB(Vector3(x0, by * s, z0), Vector3(s, s, s))
				if not _aabb_in_frustum(box, planes):
					continue
				xyz.append_array([b0.x + i, by, b0.y + j])
				bands.append(mini(int(Settings.v2m(box.get_center().distance_to(cp)) / 16.0), 2) * 16)
				seen.append(_holes_line_clear(cp, Vector3(x0 + half, clampi(hi, by * s, by * s + s), z0 + half), sm, b0 * s, n, half))
	var codes: PackedByteArray = terrain.debug_classify_mesh_blocks(xyz)
	var now := {}
	var pending := {}
	var holes := 0
	for q in codes.size():
		var c := codes[q]
		var key := Vector3i(xyz[q * 3], xyz[q * 3 + 1], xyz[q * 3 + 2])
		_holes[HOLE_KINDS[c]] = _holes.get(HOLE_KINDS[c], 0) + 1
		if c == 0:
			now[key] = true
			continue
		if c == 4:
			pending[key] = _holes_pending.get(key, t0)
			var st := str(terrain.debug_get_mesh_block_info(Vector3(key), 0).get("mesh_state", -1))
			_holes_pending_state[st] = _holes_pending_state.get(st, 0) + 1
		if c == 7 and _holes_empty_checked < 4000:
			# the mesher made nothing: fine if the block and the ring around are one material (lake water, rock)
			_holes_empty_checked += 1
			var vt: VoxelTool = terrain.get_voxel_tool()
			var inner := VoxelBuffer.new()
			inner.create(s, s, s)
			vt.copy(key * s, inner, 1 << VoxelBuffer.CHANNEL_TYPE)
			var fine := false
			if inner.is_uniform(VoxelBuffer.CHANNEL_TYPE):
				fine = inner.get_voxel(0, 0, 0, VoxelBuffer.CHANNEL_TYPE) == 0   # all air: nothing to draw
				if not fine:   # all one solid: faces only where the ring around is something else
					var ring := VoxelBuffer.new()
					ring.create(s + 2, s + 2, s + 2)
					vt.copy(key * s - Vector3i.ONE, ring, 1 << VoxelBuffer.CHANNEL_TYPE)
					fine = ring.is_uniform(VoxelBuffer.CHANNEL_TYPE)
			if fine:
				_holes["meshed_empty_uniform"] = _holes.get("meshed_empty_uniform", 0) + 1
				continue
			if _holes_empty_examples.size() < 6:
				_holes_empty_examples.append("%s inner %s" % [key * s, "uniform %d" % inner.get_voxel(0, 0, 0, VoxelBuffer.CHANNEL_TYPE) if inner.is_uniform(VoxelBuffer.CHANNEL_TYPE) else "mixed"])
		holes += 1
		_holes_by_band[str(bands[q])] = _holes_by_band.get(str(bands[q]), 0) + 1
		if seen[q]:
			_holes_visible[HOLE_KINDS[c]] = _holes_visible.get(HOLE_KINDS[c], 0) + 1
			_holes_visible_band[str(bands[q])] = _holes_visible_band.get(str(bands[q]), 0) + 1
		if _holes_prev.has(key):
			_holes_gap += 1
	_holes_prev = now
	for key in _holes_pending:   # left the pending state (or the view): how long it waited
		if not pending.has(key) and _holes_pending_ms.size() < 20000:
			_holes_pending_ms.append((t0 - _holes_pending[key]) / 1000.0)
	_holes_pending = pending
	_holes_seen += codes.size()
	_holes_series.push_back(holes)
	_holes_ms_max = maxf(_holes_ms_max, (Time.get_ticks_usec() - t0) / 1000.0)


static func _pct(v: Array[float]) -> Dictionary:
	if v.is_empty():
		return {}
	var a := v.duplicate()
	a.sort()
	return {"n": a.size(), "p50": snappedf(a[a.size() / 2], 0.1), "p90": snappedf(a[a.size() * 9 / 10], 0.1), "max": snappedf(a[-1], 0.1)}


## 1 when no true ground (surface map tops, a sample every `half` voxels from `o`) rises over the line from the
## camera to `t` before the last half block: the hole there would show (ground or a hole in front hides it).
static func _holes_line_clear(cp: Vector3, t: Vector3, sm: PackedInt32Array, o: Vector2i, n: int, half: int) -> int:
	var d := Vector2(t.x - cp.x, t.z - cp.z)
	var steps := int(d.length() / half) - 1
	for k in range(1, steps):
		var f := float(k) / (steps + 1)
		var i := floori((cp.x + d.x * f - o.x) / half)
		var j := floori((cp.z + d.y * f - o.y) / half)
		if i < 0 or j < 0 or i >= n or j >= n:
			continue
		var m := (j * n + i) * 3
		if maxi(sm[m], sm[m + 1]) > cp.y + (t.y - cp.y) * f + 2.0:
			return 0
	return 1


static func _aabb_in_frustum(box: AABB, planes: Array[Plane]) -> bool:
	for p in planes:   # normals point out of the frustum: a box wholly over any plane is outside
		var v := Vector3(box.position.x if p.normal.x > 0.0 else box.end.x, box.position.y if p.normal.y > 0.0 else box.end.y,
			box.position.z if p.normal.z > 0.0 else box.end.z)
		if p.distance_to(v) > 0.0:
			return false
	return true


## Every mesh block of the patch is drawn: meshed and up to date, or holding a visible mesh while a remesh waits
## (an edit's remesh is rate-limited, ADR 0020: the ground stays drawn meanwhile).
func _patch_drawn(patch: AABB) -> bool:
	var terrain = _main.world.terrain
	if terrain.is_area_meshed(patch, 0):
		return true
	var size := float(terrain.mesh_block_size)
	var lo := Vector3i((patch.position / size).floor())
	var hi := Vector3i(((patch.end - Vector3.ONE) / size).floor())
	for x in range(lo.x, hi.x + 1):
		for y in range(lo.y, hi.y + 1):
			for z in range(lo.z, hi.z + 1):
				var b := Vector3(x, y, z)
				if terrain.is_area_meshed(AABB(b * size, Vector3.ONE * size), 0):
					continue
				var info: Dictionary = terrain.debug_get_mesh_block_info(b, 0)
				if not (info.get("meshed", false) and info.get("visible", false)):
					return false
	return true

func _print_probe_voxels() -> void:
	for a in OS.get_cmdline_user_args():
		if a.begins_with("probe_voxels="):
			var out := []
			for t in a.substr(13).split(";", false):
				var c := t.split(",")
				var p := Vector3i(int(c[0]), int(c[1]), int(c[2]))
				out.append("%s=%s" % [p, GameMaterials.name_of(_main.world.get_voxel(p))])
			print("probe_voxels ", " ".join(out))


func _world_settled() -> bool:
	var t: Dictionary = VoxelEngine.get_stats().get("tasks", {})
	var busy: int = int(t.get("generation", 0)) + int(t.get("meshing", 0)) + int(t.get("main_thread", 0)) + int(t.get("streaming", 0))
	_idle_frames = _idle_frames + 1 if busy == 0 and _main.bubble.is_loaded() else 0
	_settled = _idle_frames >= SETTLE_FRAMES
	return _settled


func _longest_run(samples: Array) -> int:
	var best := 0
	var run := 0
	for v in samples:
		run = run + 1 if int(v) > 0 else 0
		best = maxi(best, run)
	return best


func _count_over(sorted_ms: PackedFloat64Array, limit: float) -> int:
	var i := sorted_ms.bsearch(limit, false)   # first index past every value <= limit
	return sorted_ms.size() - i


## stream_probe=1 (ADR 0135): what limits how fast the box fills in flight. Each frame checks 24 ground patches
## (4 m, from 3 m under their own ground to 1 m over it, as the detail probe) on an 8 m grid out to the render
## distance, so a sweep of the whole box takes ~10 frames; every second a row: queued generation / meshing tasks,
## meshes waiting on the upload budget, worker ms per stage in that second (ta_run_*, voxel_patch_throughput.py)
## and the last sweep's undrawn patches by 24 m band.
func _stream_probe_step(delta: float) -> void:
	var tp := Time.get_ticks_usec()
	if _sp_offsets.is_empty():
		var n := int(Settings.view_distance_m / 8.0)
		for ix in range(-n, n + 1):
			for iz in range(-n, n + 1):
				var o := Vector2(ix, iz) * 8.0
				if o.length() <= Settings.view_distance_m:
					_sp_offsets.append(Vector2(Settings.m2v(o.x), Settings.m2v(o.y)))
	var pp: Vector3 = _main.player.position
	var half := Settings.m2v(2.0)
	for k in 24:
		if _sp_i >= _sp_offsets.size():
			_sp_last = _sp_holes
			_sp_holes = {}
			_sp_i = 0
			break
		var o := _sp_offsets[_sp_i]
		_sp_i += 1
		var x := pp.x + o.x
		var z := pp.z + o.y
		var h := float(_main.world.generator.gen.height_at(int(x), int(z)))
		var patch := AABB(Vector3(x - half, h - Settings.m2v(3.0), z - half), Vector3(half * 2.0, Settings.m2v(4.0), half * 2.0))
		if not _patch_drawn(patch):
			var band := str(int(Settings.v2m(o.length()) / 24.0) * 24)
			_sp_holes[band] = int(_sp_holes.get(band, 0)) + 1
	_sp_accum += delta
	if _sp_accum >= 1.0:
		_sp_accum -= 1.0
		var t: Dictionary = VoxelEngine.get_stats().get("tasks", {})
		var ts: Dictionary = _main.world.terrain.get_statistics()
		var row := {"t": snappedf(_elapsed - _loaded_at, 0.1), "gen_q": t.get("generation", 0), "mesh_q": t.get("meshing", 0),
			"upload_q": ts.get("upload_waiting", -1), "holes": _sp_last, "patches": _sp_offsets.size()}
		for kind in ["gen", "mesh", "update", "load", "pick", "idle"]:
			var us: int = ts.get("ta_run_%s_us" % kind, 0)
			var cnt: int = ts.get("ta_run_%s_n" % kind, 0)
			row[kind + "_ms"] = snappedf((us - int(_sp_prev.get(kind + "_us", us))) / 1000.0, 0.1)
			row[kind + "_n"] = cnt - int(_sp_prev.get(kind + "_n", cnt))
			_sp_prev[kind + "_us"] = us
			_sp_prev[kind + "_n"] = cnt
		for k in ["mesh_first", "mesh_again", "mesh_edit", "mesh_gather_us", "mesh_build_us", "mesh_uniform_n", "mesh_uniform_us", "mesh_faces_us", "mesh_ao_us", "mesh_arrays_us", "mesh_occluders_us", "mesh_loop_us", "mesh_aovol_us", "mesh_merge_us", "mesh_faces_n"]:   # mesh requests by cause, mesh task phases
			var c: int = ts.get(k, 0)
			row[k] = c - int(_sp_prev.get(k, c))
			_sp_prev[k] = c
		var worst := ""   # the slowest main-thread step of this second (voxel_patch_profile.py maxima, usec; ta_u_* run in the update task)
		for k in ts:
			if String(k).begins_with("ta_") and not String(k).begins_with("ta_run") and not String(k).begins_with("ta_gen_run") and not String(k).begins_with("ta_u_") and ts[k] is int and (worst == "" or ts[k] > ts[worst]):
				worst = k
		row["ta_worst"] = "%s %d" % [worst, ts.get(worst, 0)]
		_sp_series.append(row)
	_sp_ms_max = maxf(_sp_ms_max, (Time.get_ticks_usec() - tp) / 1000.0)


## The data blocks the streaming shell's near sphere (ADR 0105: shell_near_m around the player, any direction)
## owes the player but has not loaded, by distance band in m: after settling every band inside shell_near_m is 0.
func _near_unloaded() -> Dictionary:
	var terrain = _main.world.terrain if _main != null and _main.world != null else null
	if terrain == null or not terrain.has_method("debug_get_data_block_info"):
		return {}
	var bs: int = terrain.get_data_block_size()
	var pb := Vector3i((_main.player.position / bs).floor())
	var r := int(Settings.m2v(Settings.shell_near_m)) / bs
	var by_band := {}
	var examples := []
	for z in range(-r, r + 1):
		for y in range(-r, r + 1):
			for x in range(-r, r + 1):
				var b := pb + Vector3i(x, y, z)
				var st: int = terrain.debug_get_data_block_info(Vector3(b), 0).get("loading_state", 2)
				if st == 2:
					continue
				var band := int(Settings.v2m(maxi(maxi(absi(x), absi(y)), absi(z)) * bs) / 8.0) * 8   # 0, 8, 16, 24 m
				by_band[str(band)] = by_band.get(str(band), 0) + 1
				if examples.size() < 6:
					examples.append("%s state %d" % [b * bs, st])
	return {"by_band_m": by_band, "examples": examples}


func _probe_others() -> void:
	var n: int = _host_probe()["other_godot"]
	if n > _others_max:
		_others_max = n


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