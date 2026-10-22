extends Node

## Autoload: every budget is a setting. Distances are in METRES; the world
## unit is one voxel = VOXEL_M metres (ADR 0004: 0.5 m). Use m2v()/v2m().

const VOXEL_M := 0.5
const CHUNK := 16                       # voxels per chunk edge (8 m)
const WORLD_HEIGHT_VOXELS := 768        # 384 m (ADR 0004)
const SEA_LEVEL_VOXEL := 256

# Distances (metres)
var simulation_distance_m := 24.0       # radius of the simulation bubble around the player
var render_distance_m := 64.0           # full-detail (LOD 0) radius; godot_voxel clamps lod_distance to 128 voxels = 64 m (ADR 0017), larger values are silently cut
var view_distance_m := 2048.0           # LOD horizon (vision: 2 km). ADR 0020: free at 7 LODs (LOD6 reaches 3 km with 48 m rings); an 8th LOD makes the first load 22 s (lod-7 blocks span more hydrology tiles than the generator's cache holds)
var lod_count := 7
var lod_ring_m := 48.0                  # clipbox: LOD n ends at render_distance_m + lod_ring_m * 2^n (ADR 0020: 48 m = 2-voxel cubes to 160 m, 4-voxel to 256 m; godot_voxel's default is 24 m; 64 m looks better still but 40 m/s flight then has 20-27 ms frames)
var terrain_threaded_update := true     # VoxelLodTerrain's LOD/streaming bookkeeping on a worker thread (ADR 0020: the 17 ms burst frame at 64 m rings became 9 ms)
var far_fade_start_m := 150.0           # terrain textures fade toward their tile's average colour from here (ADR 0017)
var far_fade_end_m := 600.0
var lod_fade_s := 0.35                  # dithered cross-fade when a terrain block changes LOD (ADR 0020: free with our godot_voxel build; stock godot_voxel copies every fading block and costs 30-60 ms frames)
var mesh_upload_budget_mb := 0.0        # terrain mesh data applied per frame (ADR 0020, our godot_voxel build); 0 = unlimited
var terrain_mesh_block := 32            # 16 or 32 voxels; 32 = 3x fewer draw calls (measured), coarser re-mesh granularity
var cache_generated_blocks := true      # store generated LOD data so edits cascade to far LODs (memory vs correctness)

# Simulation
var sim_tick_hz := 30.0
var sim_load_budget_ms := 0.3           # main-thread time per frame for copying terrain chunks into the bubble
var sim_push_budget_ms := 1.5           # main-thread time per frame for pasting simulated chunks back into the terrain
var sim_push_interval_ms := 0.0         # a chunk is pasted at most this often (each paste = remesh)
var terrain_mesh_collisions := false    # physics colliders for terrain meshes; bodies use voxel-grid collision instead (ADR 0011)
var swim_up_speed_m := 2.0              # holding jump in water
var sink_speed_m := 1.0                 # drifting down in water
var sim_threads := maxi(2, OS.get_processor_count() - 2)

# Player (metres, converted at use)
var walk_speed_m := 4.3
var sprint_speed_m := 7.0
var fly_speed_m := 20.0
var jump_height_m := 1.25
var step_height_m := 0.55               # auto step-up: one voxel (0.5 m) plus a little slack
var player_height_m := 1.8
var player_radius_m := 0.3
var eye_height_m := 1.62
var reach_m := 5.0
var gravity_m := 9.8 * 2.0              # Minecraft-ish snappy gravity
var mouse_sensitivity := 0.0025

# Interaction (ADR 0005)
var max_health := 20                    # half-hearts, vanilla
var fall_damage_safe_m := 3.0           # metres of fall before damage (vanilla: 3 blocks)
var pickup_radius_m := 1.4              # item entities this close to the body centre are picked up
var item_pickup_delay_s := 0.5          # a freshly dropped item cannot be picked up at once
var item_despawn_s := 300.0             # vanilla: 5 minutes
var item_merge_radius_m := 0.75         # equal stacks closer than this merge
var drop_throw_speed_m := 4.0           # Q drop velocity
var break_cooldown_s := 0.25            # vanilla: 5 ticks between broken blocks
var place_repeat_s := 0.2               # vanilla: 4 ticks between placed blocks while holding

# Game (ADR 0018)
var game_mode := "survival"             # survival | creative (gamemode=creative on the command line)
var start_minimized := true             # playtests from the editor open minimized (minimized=0 turns it off)
var start_commands: PackedStringArray = []   # run through the chat when a normal (non-autopilot) game starts
const PLAYTEST_PREFS := "user://playtest.cfg"   # the debug menu's start options

# World
var world_seed := 1337
var world_seed_text := ""               # what the player typed, if the seed came from text
var persist_world := true               # save edited chunks to user://worlds/world_<seed>_g<gen>.sqlite


## Minecraft-style seeds: a number is used as-is, any other text is hashed
## (FNV-1a, 32-bit, stable across platforms) so "epic mountains" is a valid seed.
static func seed_from_string(s: String) -> int:
	s = s.strip_edges()
	if s.is_valid_int():
		return int(s)
	var h := 2166136261
	for b in s.to_utf8_buffer():
		h = ((h ^ b) * 16777619) & 0xFFFFFFFF
	return h


func set_seed_text(s: String) -> void:
	world_seed_text = s
	world_seed = seed_from_string(s)


static func m2v(metres: float) -> float:
	return metres / VOXEL_M


static func v2m(voxels: float) -> float:
	return voxels * VOXEL_M


func sim_distance_chunks() -> int:
	return maxi(1, int(ceil(m2v(simulation_distance_m) / CHUNK)))


func render_distance_voxels() -> int:
	return int(m2v(render_distance_m))


func view_distance_voxels() -> int:
	return int(m2v(view_distance_m))


func apply_cmdline() -> Dictionary:
	## Lets Claude (or a script) override settings and request captures:
	##   godot --path game -- autopilot=1 duration=20 capture_every=1 sim_tick_hz=60 render_distance_m=64
	var args := {}
	if not OS.get_cmdline_user_args().has("autopilot=1"):
		load_playtest_prefs()   # the debug menu's start options; command-line values below still win
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		args[kv[0]] = kv[1]
		match kv[0]:
			"simulation_distance_m": simulation_distance_m = float(kv[1])
			"render_distance_m": render_distance_m = float(kv[1])
			"view_distance_m": view_distance_m = float(kv[1])
			"lod_count": lod_count = int(kv[1])
			"lod_ring_m": lod_ring_m = float(kv[1])
			"terrain_threaded_update": terrain_threaded_update = kv[1] == "1"
			"far_fade_start_m": far_fade_start_m = float(kv[1])
			"far_fade_end_m": far_fade_end_m = float(kv[1])
			"lod_fade_s": lod_fade_s = float(kv[1])
			"terrain_mesh_block": terrain_mesh_block = int(kv[1])
			"mesh_upload_budget_mb": mesh_upload_budget_mb = float(kv[1])
			"cache_generated": cache_generated_blocks = kv[1] == "1"
			"sim_tick_hz": sim_tick_hz = float(kv[1])
			"sim_load_budget_ms": sim_load_budget_ms = float(kv[1])
			"sim_push_budget_ms": sim_push_budget_ms = float(kv[1])
			"sim_push_interval_ms": sim_push_interval_ms = float(kv[1])
			"terrain_mesh_collisions": terrain_mesh_collisions = kv[1] == "1"
			"sim_threads": sim_threads = int(kv[1])
			"world_seed", "seed": set_seed_text(kv[1])
			"persist": persist_world = kv[1] == "1"
			"max_health": max_health = int(kv[1])
			"pickup_radius_m": pickup_radius_m = float(kv[1])
			"item_despawn_s": item_despawn_s = float(kv[1])
			"gamemode": game_mode = kv[1]
			"minimized": start_minimized = kv[1] == "1"
	return args


## Start options saved by the debug menu (F4): game mode, minimized playtests,
## commands to run at start. Autopilot runs never read them (reproducible).
func load_playtest_prefs() -> void:
	var cf := ConfigFile.new()
	if cf.load(PLAYTEST_PREFS) != OK:
		return
	game_mode = String(cf.get_value("start", "game_mode", game_mode))
	start_minimized = bool(cf.get_value("start", "minimized", start_minimized))
	start_commands = PackedStringArray(cf.get_value("start", "commands", PackedStringArray()))
