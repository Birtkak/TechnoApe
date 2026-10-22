extends Node

## Autoload: every budget is a setting. Distances are in METRES; the world
## unit is one voxel = VOXEL_M metres (ADR 0004: 0.5 m). Use m2v()/v2m().

const VOXEL_M := 0.5
const CHUNK := 16                       # voxels per chunk edge (8 m)
const WORLD_HEIGHT_VOXELS := 768        # 384 m (ADR 0004)
const SEA_LEVEL_VOXEL := 256

# Distances (metres)
var simulation_distance_m := 24.0       # radius of the simulation bubble around the player
var render_distance_m := 96.0           # full-detail (LOD 0) radius
var view_distance_m := 1024.0           # LOD horizon
var lod_count := 7
var terrain_mesh_block := 32            # 16 or 32 voxels; 32 = 3x fewer draw calls (measured), coarser re-mesh granularity
var cache_generated_blocks := true      # store generated LOD data so edits cascade to far LODs (memory vs correctness)

# Simulation
var sim_tick_hz := 30.0
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
			"terrain_mesh_block": terrain_mesh_block = int(kv[1])
			"cache_generated": cache_generated_blocks = kv[1] == "1"
			"sim_tick_hz": sim_tick_hz = float(kv[1])
			"sim_threads": sim_threads = int(kv[1])
			"world_seed", "seed": set_seed_text(kv[1])
			"persist": persist_world = kv[1] == "1"
	return args
