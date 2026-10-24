extends Node

## Autoload: every budget is a setting. Distances are in METRES; the world
## unit is one voxel = VOXEL_M metres (ADR 0033: 12.5 cm). Use m2v()/v2m().

const VOXEL_M := 0.125
const CHUNK := 16                       # voxels per chunk edge (2 m)
const BLOCK := 4                        # voxels per 0.5 m block edge: the size stations, furniture and recipe units keep (ADR 0034)
const WORLD_HEIGHT_VOXELS := 3072       # 384 m (ADR 0033)
const SEA_LEVEL_VOXEL := 1024           # 128 m

# Distances (metres)
var simulation_distance_m := 24.0       # nominal reach of the simulation (HUD); the window itself is sim_window_chunks
var sim_window_chunks := Vector3i(32, 16, 32)   # the wrapped simulation window: 64 x 32 x 64 m of 12.5 cm voxels (powers of two)
var sim_cell_budget := 1500000          # cells scanned per tick at most; the furthest active chunks wait (ADR 0013)
var sim_lod_m := 12.0                   # chunks further than this from the player tick every second tick
var render_distance_m := 24.0           # full-detail (LOD 0) radius. Our godot_voxel build lifts stock's 64 m clamp (ADR 0020): 96 m holds at 20 m/s (not at 40), 128 m costs p99 10 ms and 2 GB VRAM
var view_distance_m := 2048.0           # LOD horizon (vision: 2 km). ADR 0020: free at 7 LODs (LOD6 reaches 3 km with 48 m rings); an 8th LOD makes the first load 22 s (lod-7 blocks span more hydrology tiles than the generator's cache holds)
var lod_count := 8
var lod_ring_m := 24.0                  # clipbox: LOD n ends at render_distance_m + lod_ring_m * 2^n (ADR 0020: 48 m = 2-voxel cubes to 160 m, 4-voxel to 256 m; godot_voxel's default is 24 m; 64 m looks better still but 40 m/s flight then has 20-27 ms frames)
var terrain_threaded_update := true     # VoxelLodTerrain's LOD/streaming bookkeeping on a worker thread (ADR 0020: the 17 ms burst frame at 64 m rings became 9 ms)
var far_fade_start_m := 40.0            # terrain textures fade toward their tile's average colour from here (ADR 0017)
var far_fade_end_m := 200.0
var lod_fade_s := 0.35                  # dithered cross-fade when a terrain block changes LOD (ADR 0020: free with our godot_voxel build; stock godot_voxel copies every fading block and costs 30-60 ms frames)
var mesh_upload_budget_mb := 4.0        # terrain mesh data applied per frame (ADR 0020, our godot_voxel build); 0 = unlimited. 4 MB since the forests (2026-10-23): a 15 MB frame cost 16 ms
var edit_remesh_rate := 300.0           # edit-caused terrain remeshes per second, token bucket (ADR 0020, our godot_voxel build): the water spill's p99 8.3 -> 5.1 ms, 60 s of flow p99 3.8; 0 = unlimited
var terrain_mesh_block := 32            # 16 or 32 voxels; 32 = 3x fewer draw calls (measured), coarser re-mesh granularity
var cache_generated_blocks := true      # store generated LOD data so edits cascade to far LODs (memory vs correctness)
# Simulation
var sim_tick_hz := 30.0
var sim_load_budget_ms := 1.0           # main-thread time per frame for copying terrain columns into the bubble
var sim_push_budget_ms := 1.5           # main-thread time per frame for pasting simulated chunks back into the terrain
var sim_push_interval_ms := 0.0         # a chunk is pasted at most this often (each paste = remesh)
var terrain_mesh_collisions := false    # physics colliders for terrain meshes; bodies use voxel-grid collision instead (ADR 0011)
var swim_up_speed_m := 2.0              # holding jump in water
var sink_speed_m := 1.5                 # drifting down in water (to the floor, which can be walked on)
var air_s := 15.0                       # breath under water (vanilla: 300 ticks)
var drown_damage := 2                   # half-hearts per second once the air is gone (vanilla)
var sim_threads := maxi(2, OS.get_processor_count() - 2)

# Player (metres, converted at use)
var walk_speed_m := 4.3
var sprint_speed_m := 7.0
var fly_speed_m := 20.0
var jump_height_m := 1.25
var step_height_m := 0.55               # auto step-up: one 0.5 m block (BLOCK voxels) plus a little slack
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
var work_repeat_s := 0.12               # chips and clay strokes while a button is held (ADR 0023)
var pit_kiln_burn_s := 60.0             # pit kiln burn time (ADR 0023; Vintage Story's is hours of game time)
var cast_cool_s := 10.0                 # metal in a mold cools this long before it can be taken out
# Flint Age depth (ADR 0030)
var charcoal_pit_burn_s := 120.0        # a sealed pit smoulders this long (Vintage Story: about 18 game hours)
var charcoal_pit_grace_s := 20.0        # after lighting, this long to finish sealing it before the wood burns away
var firewood_per_pile := 16             # firewood in one pile voxel; two make one charcoal
var torch_lights_max := 24              # the torches nearest the player that get a real light
var torch_light_range_m := 7.0          # how far one torch lights
var torch_light_radius_m := 48.0        # torches farther from the player than this stay unlit
var pan_shakes := 6                     # right clicks to wash one pan of sand or gravel
# Crafting in the world (ADR 0042)
var metal_heat_s := 8.0                 # an ingot or work piece on a lit firepit is fully hot after this long
var metal_cool_s := 40.0                # fully hot metal is cold again this long after leaving the fire (Inventory.clock_s)
var smith_min_heat := 0.1               # below this heat the hammer does nothing: reheat the work piece
var firepit_fuel_max := 8               # fuel items a firepit holds (laid on one right click at a time)
var hold_use_s := 1.0                   # right click held this long picks up a stick or stone, lays a thing on a firepit or takes it off (ADR 0043)

# Particles (ADR 0026): two GPU pools, every particle emitted by script
# Survival (ADR 0040): hunger, body temperature, food spoilage. Seconds are real time, days game days.
var hunger_max := 20.0
var hunger_drain_s := 40.0               # one hunger point per this many seconds at rest
var hunger_fast_mult := 2.5              # sprinting or swimming
var hunger_cold_mult := 1.5              # while the body is cold
var regen_hunger := 17.0                 # at or above: one health back per regen_s
var regen_s := 6.0
var starve_s := 8.0                      # an empty bar hurts one health per this, until death
var comfort_min_c := 10.0                # environment band where the body stays at 37 C (clothing widens it later)
var comfort_max_c := 30.0
var body_ease_s := 90.0                  # the body temperature eases toward its target over about this long
var cold_c := 35.5                       # below: cold (hunger x hunger_cold_mult)
var shiver_c := 34.0                     # below: slowed to shiver_speed
var shiver_speed := 0.85
var freeze_c := 33.0                     # below: one health per freeze_s
var freeze_s := 6.0
var hot_c := 38.5
var heatstroke_c := 39.5                 # above: one health per heatstroke_s
var heatstroke_s := 8.0
var room_c := 18.0                       # a closed room pulls the temperature halfway to this
var cave_c := 10.0                       # 4 m and more under the surface
var water_c := -8.0                      # in water
var cellar_c := 8.0                      # containers 3 m and more under the surface (spoilage)
var climate_probe_s := 1.5               # how often shelter and heat are measured
var rigid_pieces := true                # cut-loose trees and limbs fall as rigid bodies (P12, ADR 0037); rigid_pieces=0 turns it off
# playtest gadgets (/kit gadgets, PlaytestTools): debug items, not progression
var laser_range_m := 64.0               # the laser cutter reaches this far
var laser_depth_m := 3.0                # and cuts this deep past the first solid voxel (through a big trunk)
var laser_pulse_s := 0.06               # one cut along the beam this often while the button is held
var laser_check_s := 0.3                # what the cuts freed is checked this often (a tree check is 24-80 ms on the sim thread)
var crater_radius_m := 3.0              # the crater rod's blast; sneaking doubles it
var particles := true                   # particles=0 turns every effect off (benchmarks against the old game)
var particle_pool_chips := 2048         # block chips alive at once at most
var particle_pool_sprites := 6144       # smoke, splashes, hearts, waterfall spray...
var particle_budget := 700              # particles emitted per frame at most; the rest of a burst is dropped
var particle_distance_m := 64.0         # bursts farther from the camera are skipped
var particle_ambient := true            # Minecraft's animateTick: lava pops, drips, spores, leaves, furnace flames
var particle_ambient_near_m := 8.0      # Minecraft: 667 random blocks within 16 blocks per tick ...
var particle_ambient_near_samples := 667
var particle_ambient_far_m := 20.0      # ... and 667 within 32; ours reaches the bubble's edge, so more samples
var particle_ambient_far_samples := 2000
var particle_waterfall_m := 90.0        # waterfall impact spray within this distance of the camera
var particle_waterfall_spray := 14.0    # droplets per second per voxel of sheet width per sqrt(metre of drop)
var particle_waterfall_burst := 3.0     # chunks of white water thrown up hard
var particle_waterfall_mist := 2.2
var particle_waterfall_foam := 4.0

# Game (ADR 0018)
var game_mode := "survival"             # survival | creative (gamemode=creative on the command line)
var start_minimized := true             # playtests from the editor open minimized (minimized=0 turns it off)
var start_commands: PackedStringArray = []   # run through the chat when a normal (non-autopilot) game starts
const PLAYTEST_PREFS := "user://playtest.cfg"   # the debug menu's start options

# Player options (ADR 0032): the Settings screen of the title and pause menus,
# saved to user://settings.cfg. Autopilot runs never read them (reproducible);
# command-line values still win. Graphics presets bundle the trade-offs that
# ADR 0031 measured; "high" is the look the budget was measured with.
signal user_settings_changed
var user_settings_path := "user://settings.cfg"   # menu_test=1 points it elsewhere
## Full detail (LOD 0) radius and LOD ring the menus allow: in a forest 24 m holds p99 6.8 ms,
## 24 m with 32 m rings 9.4 ms, 32 m 21 ms and 3.7 GB (ADR 0035).
const MIN_DETAIL_M := 16.0
const MAX_DETAIL_M := 24.0
const GRAPHICS_PRESETS := {
	"low": {"shadow_quality": "low", "ssao": false, "bloom": false, "render_distance_m": 16.0, "lod_ring_m": 16.0},
	"medium": {"shadow_quality": "medium", "ssao": true, "bloom": true, "render_distance_m": 24.0, "lod_ring_m": 24.0},
	"high": {"shadow_quality": "high", "ssao": true, "bloom": true, "render_distance_m": 24.0, "lod_ring_m": 24.0},
}
## Sun shadows per quality: Atmosphere mode, cascades, distance and filter (ADR 0031's knobs).
const SHADOW_QUALITY := {
	"off": {"mode": "off", "splits": 2, "distance_m": 0.0, "filter": 0},
	"low": {"mode": "hard", "splits": 2, "distance_m": 48.0, "filter": 0},
	"medium": {"mode": "soft", "splits": 2, "distance_m": 64.0, "filter": 1},
	"high": {"mode": "soft", "splits": 4, "distance_m": 96.0, "filter": 2},
}
const MAX_FPS_CHOICES := [60, 120, 144, 240, 0]   # 0 = unlimited
var graphics_preset := "high"           # low | medium | high | custom (a value changed by hand)
var shadow_quality := "high"            # off | low | medium | high
var ssao := true
var bloom := true
var fov := 75.0                         # degrees, vertical
var invert_y := false
var vsync := true
var max_fps := 120                      # 0 = unlimited (project default 120: 500 fps on a menu only heats the GPU)
var fullscreen := false
var volume_master := 0.8                # linear 0..1 (ADR 0044); the Audio tab
var volume_music := 0.6
var volume_effects := 1.0               # the World bus: effects and sim loops
var volume_ui := 0.7
var world_name := ""                    # the title screen's world being played (WorldList)

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
		load_user_settings()    # the player's options (ADR 0032)
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
			"edit_remesh_rate": edit_remesh_rate = float(kv[1])
			"cache_generated": cache_generated_blocks = kv[1] == "1"
			"particles": particles = kv[1] == "1"
			"rigid_pieces": rigid_pieces = kv[1] == "1"
			"particle_ambient": particle_ambient = kv[1] == "1"
			"particle_budget": particle_budget = int(kv[1])
			"sim_tick_hz": sim_tick_hz = float(kv[1])
			"sim_cell_budget": sim_cell_budget = int(kv[1])
			"sim_lod_m": sim_lod_m = float(kv[1])
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
			"graphics": apply_graphics_preset(kv[1])   # graphics=low|medium|high (ADR 0032)
	return args


## The player's options from user://settings.cfg (missing keys keep their defaults).
func load_user_settings() -> void:
	var cf := ConfigFile.new()
	if cf.load(user_settings_path) != OK:
		return
	graphics_preset = String(cf.get_value("video", "preset", graphics_preset))
	shadow_quality = String(cf.get_value("video", "shadows", shadow_quality))
	ssao = bool(cf.get_value("video", "ssao", ssao))
	bloom = bool(cf.get_value("video", "bloom", bloom))
	# 0.5 m-era files hold 48-96 m: at 12.5 cm that never finishes loading (6 GB, nothing drawn)
	render_distance_m = clampf(float(cf.get_value("video", "render_distance_m", render_distance_m)), MIN_DETAIL_M, MAX_DETAIL_M)
	lod_ring_m = clampf(float(cf.get_value("video", "lod_ring_m", lod_ring_m)), MIN_DETAIL_M, MAX_DETAIL_M)
	view_distance_m = float(cf.get_value("video", "view_distance_m", view_distance_m))
	fov = float(cf.get_value("video", "fov", fov))
	vsync = bool(cf.get_value("video", "vsync", vsync))
	max_fps = int(cf.get_value("video", "max_fps", max_fps))
	fullscreen = bool(cf.get_value("video", "fullscreen", fullscreen))
	mouse_sensitivity = float(cf.get_value("controls", "mouse_sensitivity", mouse_sensitivity))
	invert_y = bool(cf.get_value("controls", "invert_y", invert_y))
	volume_master = clampf(float(cf.get_value("audio", "master", volume_master)), 0.0, 1.0)
	volume_music = clampf(float(cf.get_value("audio", "music", volume_music)), 0.0, 1.0)
	volume_effects = clampf(float(cf.get_value("audio", "effects", volume_effects)), 0.0, 1.0)
	volume_ui = clampf(float(cf.get_value("audio", "interface", volume_ui)), 0.0, 1.0)
	if not SHADOW_QUALITY.has(shadow_quality):
		shadow_quality = "high"
	graphics_preset = matching_preset()


func save_user_settings() -> void:
	var cf := ConfigFile.new()
	cf.set_value("video", "preset", graphics_preset)
	cf.set_value("video", "shadows", shadow_quality)
	cf.set_value("video", "ssao", ssao)
	cf.set_value("video", "bloom", bloom)
	cf.set_value("video", "render_distance_m", render_distance_m)
	cf.set_value("video", "lod_ring_m", lod_ring_m)
	cf.set_value("video", "view_distance_m", view_distance_m)
	cf.set_value("video", "fov", fov)
	cf.set_value("video", "vsync", vsync)
	cf.set_value("video", "max_fps", max_fps)
	cf.set_value("video", "fullscreen", fullscreen)
	cf.set_value("controls", "mouse_sensitivity", mouse_sensitivity)
	cf.set_value("controls", "invert_y", invert_y)
	cf.set_value("audio", "master", volume_master)
	cf.set_value("audio", "music", volume_music)
	cf.set_value("audio", "effects", volume_effects)
	cf.set_value("audio", "interface", volume_ui)
	cf.save(user_settings_path)


## Sets every value of a graphics preset (low / medium / high).
func apply_graphics_preset(p_name: String) -> void:
	if not GRAPHICS_PRESETS.has(p_name):
		return
	graphics_preset = p_name
	for k in GRAPHICS_PRESETS[p_name]:
		set(k, GRAPHICS_PRESETS[p_name][k])


## The preset whose values match the current ones, or "custom".
func matching_preset() -> String:
	for p in GRAPHICS_PRESETS:
		var same := true
		for k in GRAPHICS_PRESETS[p]:
			if get(k) != GRAPHICS_PRESETS[p][k]:
				same = false
		if same:
			return p
	return "custom"


## Window, vsync and frame cap (not for autopilot runs: DebugCapture owns those).
func apply_display() -> void:
	if OS.get_cmdline_user_args().has("autopilot=1") or DisplayServer.get_name() == "headless":
		return
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_ENABLED if vsync else DisplayServer.VSYNC_DISABLED)
	Engine.max_fps = max_fps
	var want := DisplayServer.WINDOW_MODE_FULLSCREEN if fullscreen else DisplayServer.WINDOW_MODE_WINDOWED
	if DisplayServer.window_get_mode() != want and not (not fullscreen and DisplayServer.window_get_mode() == DisplayServer.WINDOW_MODE_MAXIMIZED):
		DisplayServer.window_set_mode(want)


## After the Settings screen changed something: save, apply the display, tell the game.
func user_settings_edited() -> void:
	graphics_preset = matching_preset()
	save_user_settings()
	apply_display()
	user_settings_changed.emit()


## Start options saved by the debug menu (F4): game mode, minimized playtests,
## commands to run at start. Autopilot runs never read them (reproducible).
func load_playtest_prefs() -> void:
	var cf := ConfigFile.new()
	if cf.load(PLAYTEST_PREFS) != OK:
		return
	game_mode = String(cf.get_value("start", "game_mode", game_mode))
	start_minimized = bool(cf.get_value("start", "minimized", start_minimized))
	start_commands = PackedStringArray(cf.get_value("start", "commands", PackedStringArray()))
