extends Node

## Autoload: every budget is a setting. Distances are in METRES; the world
## unit is one voxel = VOXEL_M metres (ADR 0033: 12.5 cm). Use m2v()/v2m().

const VOXEL_M := 0.125
const CHUNK := 16                       # voxels per chunk edge (2 m)
const BLOCK := 4                        # voxels per 0.5 m block edge: the size stations, furniture and recipe units keep (ADR 0034)
const WORLD_HEIGHT_VOXELS := 5120       # 640 m (ADR 0051: massive mountains; was 384 m, ADR 0033)
const SEA_LEVEL_VOXEL := 1024           # 128 m

# Distances (metres)
var simulation_distance_m := 24.0       # nominal reach of the simulation (HUD); the window itself is sim_window_chunks
var sim_window_chunks := Vector3i(32, 32, 32)   # the wrapped simulation window: 64 x 64 x 64 m of 12.5 cm voxels (powers of two; 64 m tall since ADR 0050)
var sim_cell_budget := 1500000          # cells scanned per tick at most; the furthest active chunks wait (ADR 0013)
var sim_lod_m := 12.0                   # chunks further than this from the player tick every second tick
var ground_shell := true                # ADR 0105: the full-detail box follows the ground (shell=0: the whole cube)
var shell_below_m := 4.0                # ... from this far under the lowest ground of a 6 m patch (an opening's own depth where a cave opens to the sky, ADR 0124; 8 until 2026-11-03)
var shell_above_m := 64.0               # ... to this far over its highest ground, water or tree
var shell_near_m := 32.0                # ... plus everything this close to the player, any direction
var shadow_caster_m := 40.0             # ADR 0105: terrain farther than this casts no sun shadow (the shadow pass drew 2/3 of the triangles)
var merge_faces := true                 # ADR 0106: the mesher merges unshaded cube faces into rectangles (fewer triangles, same picture)
var ao_pages := true                    # ADR 0107: merged faces keep their corner occlusion in texture pages (every cube face merges)
var view_priority_weight := 3.0         # ADR 0136: terrain tasks behind the camera's view wait: priority distance x (1 + w (1 - cos angle off the view)), off under a loading cover; 0 = distance only
var ao_page_layers := 0                 # ADR 0107: pages (256x256 R8 = 64 KB each), one per mesh block in view; 0 = sized from the render distance
var view_distance_m := 72.0             # the horizon = the radius of the one box of full-detail voxels (ADR 0073: no LOD; 72 m holds the budget in flight since merged faces, ADR 0106). The terrain blends into the sky over the last quarter
var horizon_fade_frac := 0.75           # the terrain blends into the sky from this fraction of view_distance_m to the edge (terrain_common.gdshaderinc)
var terrain_threaded_update := true     # VoxelLodTerrain's streaming bookkeeping on a worker thread (ADR 0020: the 17 ms burst frame became 9 ms)
var far_fade_start_m := 28.0            # terrain textures fade toward their tile's average colour from here (ADR 0017): flat colour by the horizon blend
var far_fade_end_m := 40.0
var mesh_upload_budget_mb := 4.0        # terrain mesh data applied per frame (ADR 0020, our godot_voxel build); 0 = unlimited. 4 MB since the forests (2026-10-23): a 15 MB frame cost 16 ms
var edit_remesh_rate := 300.0           # edit-caused terrain remeshes per second, token bucket (ADR 0020, our godot_voxel build): the water spill's p99 8.3 -> 5.1 ms, 60 s of flow p99 3.8; 0 = unlimited
var edit_remesh_rate_busy := 60.0       # ... while the sim's push backlog is over remesh_busy_queue chunks (a forest fire: ~1600): each applied remesh costs the main thread ms, 300/s gave p99 ~48 ms, 60/s ~32
var edit_remesh_free_s := 0.5          # no edit remesh limit this long after a big player edit (laser, creative brush): at 300/s a wide laser left 2 s of stale meshes, unlimited 0.5 s, worst frame 185 -> 50 ms
var remesh_busy_queue := 256            # a 35k-voxel water fill peaks at ~90 queued chunks
var terrain_mesh_block := 64            # 16, 32 or 64 voxels (64: our godot_voxel build, ADR 0105): 64 = ~5x fewer draw calls than 32 at 72-100 m; an edit remeshes a 8 m block
var cache_generated_blocks := true      # keep generated voxel data in memory (edits write into it; off = memory for re-generation)
# Simulation
var sim_tick_hz := 30.0
var sim_column_flow := true             # ADR 0119 A: moving liquids level as columns by head (column_flow=0: the eighth kernel of ADR 0069)
var sim_states := true                  # ADR 0119 C: heat, pressure and state switching (states=0 turns them off)
var sim_gravity_x := 3.0                # falling sand and water: real gravity rounds to 1/16 cell/tick^2, a stream took 2 s to reach speed
var sim_load_budget_ms := 1.0           # main-thread time per frame for copying terrain columns into the bubble
var sim_push_budget_ms := 1.5           # main-thread time per frame for pasting simulated chunks back into the terrain
var sim_push_interval_ms := 0.0         # a chunk is pasted at most this often (each paste = remesh)
var terrain_mesh_collisions := false    # physics colliders for terrain meshes; bodies use voxel-grid collision instead (ADR 0011)
var swim_up_speed_m := 2.0              # holding jump in water
var sink_speed_m := 1.5                 # drifting down in water (to the floor, which can be walked on)
var air_s := 15.0                       # breath under water (vanilla: 300 ticks)
var suffocate_s := 2.0                  # breath with the head buried in sand or gravel (vanilla: damage at once)
var drown_damage := 2                   # half-hearts per second once the air is gone (vanilla)
var sim_threads := maxi(2, OS.get_processor_count() - 2)
var voxel_threads := maxi(2, OS.get_processor_count() - 4)   # godot_voxel workers while playing (ADR 0135: its default, half the cores, fell behind a 20 m/s flight; 8 of 12 on the dev box hold the frame budget)

# Player (metres, converted at use)
var walk_speed_m := 4.3
var sprint_speed_m := 7.0               # Ctrl (Minecraft; bug report 2026-10-25)
var sneak_speed_m := 1.3                # Shift: Minecraft's sneak is 0.3 x walking
var sneak_eye_drop_m := 0.25            # and lowers the eyes
var fly_speed_m := 20.0
var jump_height_m := 1.25
var climb_speed_m := 2.4                # up or down a ladder (Minecraft's 2.35 blocks/s; ADR 0094)
var step_height_m := 0.55               # auto step-up: one 0.5 m block (BLOCK voxels) plus a little slack
var small_step_m := 0.3                 # ... with auto_step off: rough ground (two voxels) still walks, a full block needs a jump
var player_height_m := 1.8
var player_radius_m := 0.3
var eye_height_m := 1.62
var reach_m := 5.0
var creative_reach_m := 12.0            # creative world editing: big brushes need room (ADR 0065)
var gravity_m := 9.8 * 2.0              # Minecraft-ish snappy gravity
var mouse_sensitivity := 0.0025

# Interaction (ADR 0005)
var max_health := 20                    # half-hearts, vanilla
var fall_damage_safe_m := 3.0           # metres of fall before damage (vanilla: 3 blocks)
var pickup_radius_m := 2.2              # item entities this close to the body centre are picked up (1.4 until bug report 2026-10-25: "a bit further")
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
var ingot_pile_max := 64                # ingots in one 0.5 m pile, as in Vintage Story (ADR 0095)
var torch_lights_max := 24              # the light sources nearest the player that get a real light (TorchLights)
var torch_light_range_m := 16.0         # how far one torch lights (2026-10-25: 7 m read as a small glow; ADR 0053)
var torch_light_energy := 1.8           # a torch's light at its flame (1.3 read as too soft, bug 2026-10-27_15-42-49)
var torch_light_attenuation := 0.7      # OmniLight3D falloff: < 1 spreads the light and fades it gently to the range
var torch_light_radius_m := 64.0        # light sources farther from the player than this stay unlit
var fire_light_range_m := 18.0          # a lit firepit, furnace, kiln or firewood pile (TorchLights)
var fire_light_energy := 1.6
var lava_light_range_m := 14.0          # lava lights the rock around it (TorchLights): one light per lava_light_grid_m cell
var lava_light_energy := 1.4
var lava_light_grid_m := 4.0
var lava_lights_max := 8                # of torch_lights_max: a lava sea never takes every light from the torches
var pan_shakes := 6                     # right clicks to wash one pan of sand or gravel
# Crafting in the world (ADR 0042)
var metal_heat_s := 8.0                 # an ingot or work piece on a lit firepit is fully hot after this long
var metal_cool_s := 40.0                # fully hot metal is cold again this long after leaving the fire (Inventory.clock_s)
var smith_min_heat := 0.1               # below this heat the hammer does nothing: reheat the work piece
var metal_hot_c := 1200.0              # degrees C of fully hot metal; cold is 20 (ADR 0087: the tooltip and look-at readout)
var glow_from_heat := 0.4               # heat (~500 C) where metal starts to glow dull red in icons and models (ADR 0087)
var forge_input_max := 2                # items (iron ingots) a forge holds at once, laid side by side on the coals (ADR 0087)
var hot_hand_damage := 2                # hot metal in a hand without tongs in the other: dropped, and this much damage (ADR 0087)
var firepit_fuel_max := 8               # fuel items a firepit holds (laid on one right click at a time)
# The steam age (ADR 0097): pressure is a fraction of the small boiler's working pressure
var boiler_bucket_fill := 0.25          # of the tank one water bucket fills
var boiler_boil_s := 240.0              # a full tank boils away in this long while lit
var boiler_build_s := 20.0              # from empty to working pressure (1.0) in this long while lit with water
var boiler_burst_at := 1.3              # pressure that blows the boiler up; the whistle screams above 1.0
var boiler_dry_s := 8.0                 # lit and dry: the whistle, then after this long the blast
var boiler_wear_s := 1200.0             # lit this long and the boiler is rusted through (an iron plate mends it); steam falls with wear
var boiler_blast_m := 4.0               # within this a player is hurt by the blast
var boiler_blast_damage := 8            # halved per steam_proof piece worn (goggles, apron)
var pipe_steam_cap := 1.0               # ADR 0122: bar a second a bronze pipe carries (copper half); a run is capped by its weakest pipe
var pipe_water_cap := 2.0               # ADR 0122: buckets a second a bronze pipe carries (copper half)
var steam_per_use := 0.02               # pressure one hammer strike or sieve shake draws
var steam_hammer_strike_s := 0.5        # between strikes at working pressure; faster above, slower below
var steam_sieve_shake_s := 0.6          # between shakes at working pressure
var steam_sieve_loads := 8              # blocks of gravel or sand a steam sieve holds
# stage 2 and the turbine (ADR 0099)
var steel_boiler_x := 2.5               # the steel boiler builds pressure this many times faster than the small one
var tank_buckets := 16                  # water buckets a water tank holds
var ore_crusher_crush_s := 1.0          # between crushes at working pressure
var ore_crusher_loads := 16             # blocks of ore an ore crusher holds
var turbine_rpm := 600.0                # a turbine's speed at working pressure; its power scales with it (ADR 0113)
var lv_volts := 48.0                    # ADR 0123: the one tier, 48 V DC (extra-low voltage: safe to touch, telecom and workshop standard)
var dynamo_volts := 52.0                # ... the turbine's dynamo EMF at working speed (sags under load through dynamo_ohms)
var dynamo_ohms := 0.3                  # ... its internal resistance: at most ~2.2 kW, five 400 W machines before the voltage drops out
var battery_volts_empty := 46.0         # ... lead-acid cells: EMF from empty to full
var battery_volts_full := 50.0
var battery_ohms := 0.1
var lv_start_frac := 0.95               # ... a load starts when its node holds this share of lv_volts (one start a second per circuit)
var lv_stop_frac := 0.85                # ... and stops under this share, then waits lv_retry_s before trying again
var lv_retry_s := 5
var lv_retry_max_s := 60                # ... the wait doubles on every stop up to this, and resets after a steady minute
var lv_brownout_frac := 0.7             # ... a load that is not the newest on its circuit holds on down to this share
var cable_burn_s := 10                  # ... seconds over its rating before a cable burns out
var circuit_device_ohms := 0.001        # ... a switch, fuse or breaker in line
var crank_s := 20.0                     # ... a right click turns the hand crank this long (about 60 W, what a person keeps up)
var lamp_light_range_m := 12.0          # ADR 0117: an electric lamp's light (TorchLights), paler and smaller than a fire
var lamp_light_energy := 1.2
var pump_buckets := 4                   # ... and its tank, in buckets (1 m^3 each)
var builder_voxels_per_s := 48.0        # ADR 0161: voxels (and pieces) a powered builder places a second
var builder_reach_m := 24.0             # ... how far from a site it may stand
var machine_pins_max := 2               # ADR 0117: pins kept alive for machines that need the simulation (pumps), nearest first
var waste_port_cells := 256             # ADR 0141: waste cells a machine holds for its port; at this it stalls (port blocked)
var waste_cells_per_s := 16             # ... cells of each waste a port lets out a second (into free cells only)
var waste_intake_buckets := 4           # ... sludge a scrubber holds, in buckets
var chimney_max_m := 16.0               # ... the tallest chimney stack the vent search follows
var wind_cells_per_tick := 0.6          # ... a full wind's drift for smoke (SimWorld.set_wind); WorkSystem's wind 0..1 scales it
var jetpack_rise_m := 2.0               # the steam jetpack climbs this fast with jump held
var jetpack_sink_m := 0.6               # ...and sinks this fast with it released (a hover)
var jetpack_air_s := 90.0               # seconds of climbing one fill of compressed air gives (hovering costs a quarter)
var compressor_fill_s := 60.0           # a compressor fills its air tank in this long at working pressure
# movement toys (ADR 0151)
var glider_trim_speed_m := 9.0          # the glider's airspeed looking level (it settles there)
var glider_glide_ratio := 8.0           # metres forward per metre down at trim (a 1.1 m/s sink)
var glider_max_speed_m := 28.0          # a dive's speed is capped here
var rope_max_m := 32.0                  # a climbing rope hangs at most this far
var grapple_range_m := 36.0             # the hook flies this far before it comes back
var grapple_speed_m := 70.0             # ...this fast
var grapple_reel_m := 14.0              # held right click shortens the line this fast
# stage 3 transport (ADR 0100)
var hopper_s := 1.0                     # a hopper or chute moves one item this often
var belt_s := 1.0                       # a belt carries an item one block in this long
var minecart_speed_m := 8.0             # a ridden minecart's speed
var hold_use_s := 1.0                 # right click held this long picks up a stick or stone, lays a thing on a firepit or takes it off (ADR 0043)

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
# the four Stone Age stages (ADR 0059)
var tinder_fungus_chance := 0.08        # a peel of birch bark brings tinder fungus with it
var net_catch_chance := 0.35            # a cast of a fishing net brings up a fish
var fish_bite_min_h := 0.3              # a fishing line bites after this many game hours (0.3 h = 15 s)
var fish_bite_max_h := 1.2
var canoe_thrust_m := 2.2               # paddling: m/s per second along the bow
var canoe_turn_rad := 1.2               # turning: rad/s at full A or D
var canoe_drag := 0.6                   # speed lost per second (exp), so it glides on and slows
var canoe_current_m := 0.8              # a river's push downstream (m/s)
var dugout_min_m := 2.5                 # a canoe needs a trunk at least this long
var dugout_burn_h := 3.0                # game hours the fire takes to burn out its hollow
var tame_meat := 3                      # pieces of raw meat that tame a wolf
var body_ease_s := 90.0                  # the body temperature eases toward its target over about this long
var cold_c := 35.5                       # below: cold (hunger x hunger_cold_mult)
var shiver_c := 34.0                     # below: slowed to shiver_speed
var shiver_speed := 0.85
var freeze_c := 33.0                     # below: one health per freeze_s
var freeze_s := 6.0
var hot_c := 38.5
var heatstroke_c := 39.5                 # above: one health per heatstroke_s
var heatstroke_s := 8.0
var poison_every_s := 1.25               # poisoned (a dart frog's touch): one health per this, never the last one (Minecraft)
# ADR 0155: pollution pushes back. Smoke: a native count of smoke + toxic gas cells in a box round the head.
var smoke_box_m := 0.4                   # half size of that box (0.4 m = 3 voxels: a 7x7x7 box)
var smoke_dense_cells := 60              # this many gas cells in the box (of 343) is dense smoke
var smoke_cough_after_s := 2.0           # breathing dense smoke this long: the first cough, the screen greys
var smoke_cough_every_s := 3.0
var smoke_hurt_after_s := 8.0            # then one health per smoke_hurt_every_s (it can kill: get out or mask up)
var smoke_hurt_every_s := 4.0
var crop_sick_growth := 0.25             # a sickly crop grows at this share of its speed
var crop_wilt_ticks := 3                 # polluted random ticks in a row (minus clean ones) before it wilts to a dead bush
var crop_smoke_cells := 2                # smoke or toxic gas in this many of the 4 voxels over a crop is heavy smoke
var fish_gasp_s := 6.0                   # in fouled water this long: a fish rises to gasp at the surface (the sign)
var fish_foul_die_s := 24.0              # this long: it dies and floats belly-up for fish_float_s
var fish_float_s := 30.0
var room_c := 18.0                       # a closed room pulls the temperature halfway to this
var cave_c := 10.0                       # 4 m and more under the surface
var water_c := -8.0                      # in water
var cellar_c := 8.0                      # containers 3 m and more under the surface (spoilage)
# The survival loop (ADR 0061): root cellars, preserved food, frost, prospecting
var root_cellar_m := 1.5                 # a closed room this far under the surface is a root cellar
var root_cellar_mult := 0.3              # containers there spoil at the cellar's 8 C times this
var comfort_wall_m := 4.0                # home comfort (ADR 0162): a wall this near counts, rays from the bed
var comfort_roof_m := 3.5                # a roof this high counts
var comfort_light_m := 6.0               # lamps, fires and furniture count inside this
var comfort_heat := 5.0                  # entity heat at the bed for the full warmth share (a lit firepit at 2 m)
var comfort_rest_min := 45               # points a room needs to rest you
var comfort_rested_s := 900.0            # the Rested buff after a night in a perfect room
var rested_hunger_cut := 0.35            # Rested at amount 1: hunger drains this much slower
var rested_speed := 0.05                 # Rested at amount 1: walking this much faster
var preserved_spoil_mult := 0.25         # foods tagged "preserved" (salted, dried, smoked) spoil this much as fast
var frost_check_s := 10.0                # how often crops are checked for frost
var prospect_depth_m := 12.0             # density mode: how far down under the struck spot the pick counts rock (ADR 0147)
var prospect_width_m := 3.0              # ... and how wide the square column is
var discovery_check_s := 1.0             # how often the discovery journal looks around the player
var climate_probe_s := 1.5               # how often shelter and heat are measured
var bucket_m3 := 1.0                      # a bucket holds this much liquid: 512 voxels, a 1 m pit (ADR 0091)
var tunnel_width_m := 1.0                # the tunnel brush: a hallway the 0.6 m wide, 1.8 m tall body walks (ADR 0091)
var tunnel_height_m := 2.0
# maps (plan item D10, ADR 0055)
var quick_stack_m := 8.0                # the inventory's Stack button reaches chests this close (plan item D11)
var map_explore_m := 48.0               # the map fills in this far around the player
var waypoint_beam_m := 64.0             # a waypoint's beam rises this high
var minimap_corner := "top_right"       # top_right | top_left | bottom_right | bottom_left (Settings screen)
var minimap_size_px := 160              # 0 hides the minimap
# quality of life (ADR 0130): each one has its own switch in Settings, where taste differs
var toggle_sprint := false              # Ctrl latches sprinting on and off instead of being held
var toggle_sneak := false               # Shift latches sneaking on and off instead of being held
var hud_coords := false                 # a line top left: position in metres and the facing (compass)
var zoom_factor := 4.0                  # holding the zoom key (Z) narrows the view this much (OptiFine's zoom)
var auto_step := true                   # walk up full 0.5 m blocks (Minecraft's auto-jump); off: jump onto them
var screen_effects := true              # a red edge when hurt, frost or heat at the edges when the body is cold or hot
# weather and forest fires (ADR 0056; the developer chose "moderate" fire and random weather + fire rain)
var weather := true                     # weather=0: always clear (benchmarks); weather=rain|clear starts with it
var weather_roll_s := 600.0             # a random shower is rolled about this often ...
var weather_rain_chance_dry := 0.08     # ... with this chance in a dry column (humidity 0) ...
var weather_rain_chance_wet := 0.55     # ... and this in a wet one (humidity 1)
var weather_rain_s := 180.0             # a shower lasts about this long
var weather_fade_s := 8.0               # clouds and rain come and go over this long
var weather_particles := 4000           # rain or snow particles around the camera (amount_ratio follows the intensity)
var weather_scan_s := 0.5               # the sim is sampled for fire this often ...
var weather_fire_scan_m := 24.0         # ... within this radius ...
var weather_fire_samples := 4000        # ... with this many random cells (flames, smoke, burning cells; about 0.5 ms)
var weather_fire_clear_s := 20.0        # the fire rain stops once no fire was seen for this long
var fire_rain_delay_min_s := 60.0       # a forest fire brings rain after 60-180 s (a region burns, not the map)
var fire_rain_delay_max_s := 180.0
var rain_drops_per_s := 160.0           # water cells dropped on the fire cells the scans found while it rains (one each)
var rain_db := -10.0                    # the rain loop at full rain
var rigid_pieces := true                # cut-loose trees and limbs fall as rigid bodies (P12, ADR 0037); rigid_pieces=0 turns it off
# ADR 0142, mines need shoring (MineSupport): a dug roof cell must be within its rock's span of a wall or under a prop
var cave_ins := true                    # cave_ins=0 turns it off (needs rigid_pieces)
var cave_ins_all_modes := false         # creative and spectator digs count too (/support modes all); survival only by default
var support_span_base_m := 1.0          # a roof cell may lie base + per_hardness x its mining hardness from a wall:
var support_span_per_hardness_m := 1.0  # sandstone 1.8 m, stone 2.5 m, ore 4 m (a 3 m tunnel always stands)
var support_span_max_m := 4.0
var support_warn_min_s := 15.0          # creak to fall: a roof far past its span goes this soon ...
var support_warn_max_s := 120.0         # ... one just past it creaks this long (time to prop it)
var screen_shake := true                # the last seconds before a roof falls shake the camera (screen_shake=0)
var support_scan_s := 0.25             # digs and prop changes are checked together this often
# playtest gadgets (/kit gadgets, PlaytestTools): debug items, not progression
var laser_range_m := 64.0               # the laser cutter reaches this far
var laser_depth_m := 3.0                # and cuts this deep past the first solid voxel (through a big trunk)
var laser_pulse_s := 0.06               # one cut along the beam this often while the button is held
var laser_check_s := 0.3                # what the cuts freed is checked this often (a tree check is 24-80 ms on the sim thread)
var crater_radius_m := 3.0              # the crater rod's blast; sneaking doubles it
# TNT (ADR 0065): one 0.5 m block of it; a part-filled block blasts with the cube root of its share
var tnt_radius_m := 2.5                 # crater radius in soft ground; stone takes 3/4 of it, hard rock 1/2, obsidian none
var tnt_fuse_s := 3.0                   # lit by hand, fire or the igniter
var tnt_chain_fuse_s := Vector2(0.1, 0.3)   # lit by another blast: a random fuse in this range (Minecraft 0.5-1.5 s; faster so a pile goes off as one)
var tnt_merge_s := 0.3                  # a charge going off takes along primed charges in its reach with this little fuse left: one bigger blast
var tnt_damage := 16                    # health at the centre, falling to 0 at twice the radius (armour later)
var tnt_knockback_m := 12.0             # m/s at the centre, falling like the damage
var tnt_drop_fraction := 0.3            # share of the blasted voxels that drops as items (Minecraft: 1 / power)
var fuse_speed_mps := 0.5               # fuse cord burn speed (ADR 0150): sets the cord's ignite_chance, at most tick_hz / 6 voxels a second
var tnt_debris_max := 24                # flung debris pieces per blast (ADR 0150), out of the voxels the blast removed
var tnt_debris_live_max := 96           # flung pieces in the air at once, all blasts together
var tnt_blasts_per_frame := 1           # fuses that run out together wait their turn: one crater per frame
var igniter_range_m := 16.0             # the igniter's sneak use lights every charge this close
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
## View distances the menu allows (ADR 0073, measured at seed 272905368, vsync off, ADR 0014 method):
## 32 m p99 4.6 ms / 0.6 GB VRAM, 40 m p99 6.2 ms / 0.9 GB, 48 m p99 32 ms standing (fails) / 1.3 GB.
const MIN_VIEW_M := 24.0
const MAX_VIEW_M := 100.0
const VIEW_CHOICES_M := [24.0, 32.0, 40.0, 56.0, 72.0, 100.0]   # 100 m holds the budget in flight, a new world loads ~20 s (ADR 0105 amendment)
const GRAPHICS_PRESETS := {
	"low": {"shadow_quality": "low", "ssao": false, "bloom": false, "view_distance_m": 24.0},
	"medium": {"shadow_quality": "medium", "ssao": true, "bloom": true, "view_distance_m": 40.0},
	"high": {"shadow_quality": "high", "ssao": true, "bloom": true, "view_distance_m": 72.0},
}
## Sun shadows per quality: Atmosphere mode, cascades, distance and filter (ADR 0031's knobs).
const SHADOW_QUALITY := {
	"off": {"mode": "off", "splits": 2, "distance_m": 0.0, "filter": 0},
	"low": {"mode": "hard", "splits": 2, "distance_m": 48.0, "filter": 0},
	"medium": {"mode": "soft", "splits": 2, "distance_m": 64.0, "filter": 1},
	"high": {"mode": "soft", "splits": 4, "distance_m": 96.0, "filter": 2},
}
const GUI_SCALES := [0.75, 1.0, 1.25, 1.5, 1.75, 2.0]   # HUD and menus (window content_scale_factor; the 3D view keeps its resolution)
const JEI_COLS := [0, 4, 6, 8, 10]   # recipe browser list width in columns; 0 = fill the room beside the panels
const MAX_FPS_CHOICES := [60, 120, 144, 240, 0]   # 0 = unlimited
var graphics_preset := "high"           # low | medium | high | custom (a value changed by hand)
var shadow_quality := "high"            # off | low | medium | high
var ssao := true
var bloom := true
var fog := true                          # the haze and lowland mist above ground (bug 2026-10-27_15-49-06; caves and water keep theirs)
var brightness := 0.5                   # 0 moody .. 1 bright; 0.5 is the graded look (Atmosphere.brightness_mult)
var fov := 75.0                         # degrees, vertical
var invert_y := false
var brush_ghost := true   # the shell of the cells a dig or placement will touch (ADR 0065; off in Settings)
var gui_scale := 1.0                     # one of GUI_SCALES; applies live (apply_display)
var jei_shown := true                    # the recipe browser list beside the inventory
var jei_cols := 0                        # one of JEI_COLS
var vsync := true
var max_fps := 120                      # 0 = unlimited (project default 120: 500 fps on a menu only heats the GPU)
var fullscreen := false
var volume_master := 0.8                # linear 0..1 (ADR 0044); the Audio tab
var volume_music := 0.6
var volume_effects := 1.0               # the World bus: effects and sim loops
var volume_ui := 0.7
var world_name := ""                    # the title screen's world being played (WorldList)
var net_address := "127.0.0.1"          # the title screen's last server address and player name (ADR 0145)
var net_name := "player"

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


## The voxels the player's body steps up on its own (ADR 0130: auto_step off keeps only small steps).
func step_voxels() -> float:
	return m2v(step_height_m if auto_step else small_step_m)


static func m2v(metres: float) -> float:
	return metres / VOXEL_M


static func v2m(voxels: float) -> float:
	return voxels * VOXEL_M


func sim_distance_chunks() -> int:
	return maxi(1, int(ceil(m2v(simulation_distance_m) / CHUNK)))


func view_distance_voxels() -> int:
	return int(m2v(view_distance_m))


func apply_cmdline() -> Dictionary:
	## Lets Claude (or a script) override settings and request captures:
	##   godot --path game -- autopilot=1 duration=20 capture_every=1 sim_tick_hz=60 view_distance_m=32
	var args := {}
	if not OS.get_cmdline_user_args().has("autopilot=1"):
		load_user_settings()    # the player's options (ADR 0032)
		load_playtest_prefs()   # the debug menu's start options; command-line values below still win
		TunePanel.load_tune()   # the F6 tuning panel's saved values (user://tune.cfg)
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		args[kv[0]] = kv[1]
		match kv[0]:
			"simulation_distance_m": simulation_distance_m = float(kv[1])
			"view_distance_m": view_distance_m = float(kv[1])
			"shell": ground_shell = kv[1] == "1"
			"shadow_caster_m": shadow_caster_m = float(kv[1])
			"merge_faces": merge_faces = kv[1] == "1"
			"ao_pages": ao_pages = kv[1] == "1"
			"shell_below_m": shell_below_m = float(kv[1])
			"shell_above_m": shell_above_m = float(kv[1])
			"shell_near_m": shell_near_m = float(kv[1])
			"view_priority": view_priority_weight = float(kv[1])
			"terrain_threaded_update": terrain_threaded_update = kv[1] == "1"
			"far_fade_start_m": far_fade_start_m = float(kv[1])
			"far_fade_end_m": far_fade_end_m = float(kv[1])
			"terrain_mesh_block": terrain_mesh_block = int(kv[1])
			"mesh_upload_budget_mb": mesh_upload_budget_mb = float(kv[1])
			"edit_remesh_rate": edit_remesh_rate = float(kv[1])
			"column_flow": sim_column_flow = kv[1] == "1"
			"states": sim_states = kv[1] == "1"
			"edit_remesh_free_s": edit_remesh_free_s = float(kv[1])
			"cache_generated": cache_generated_blocks = kv[1] == "1"
			"particles": particles = kv[1] == "1"
			"rigid_pieces": rigid_pieces = kv[1] == "1"
			"cave_ins": cave_ins = kv[1] == "1"
			"screen_shake": screen_shake = kv[1] == "1"
			"support_warn_s":   # support_warn_s=min,max (test runs)
				var mm := kv[1].split(",")
				support_warn_min_s = float(mm[0])
				support_warn_max_s = float(mm[mm.size() - 1])
			"weather": weather = kv[1] != "0"
			"fire_rain_delay_s":
				fire_rain_delay_min_s = float(kv[1])
				fire_rain_delay_max_s = float(kv[1])
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
			"voxel_threads": voxel_threads = int(kv[1])
			"world_seed", "seed": set_seed_text(kv[1])
			"persist": persist_world = kv[1] == "1"
			"max_health": max_health = int(kv[1])
			"pickup_radius_m": pickup_radius_m = float(kv[1])
			"item_despawn_s": item_despawn_s = float(kv[1])
			"gamemode": game_mode = kv[1]
			"minimized": start_minimized = kv[1] == "1"
			"gui_scale":
				gui_scale = clampf(float(kv[1]), GUI_SCALES[0], GUI_SCALES[-1])
				get_window().content_scale_factor = gui_scale
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
	fog = bool(cf.get_value("video", "fog", fog))
	brightness = clampf(float(cf.get_value("video", "brightness", brightness)), 0.0, 1.0)
	# files from before ADR 0073 hold 200 m or 1-2 km of LOD horizon: at 12.5 cm one box of that never loads
	view_distance_m = clampf(float(cf.get_value("video", "view_distance_m", view_distance_m)), MIN_VIEW_M, MAX_VIEW_M)
	fov = float(cf.get_value("video", "fov", fov))
	vsync = bool(cf.get_value("video", "vsync", vsync))
	max_fps = int(cf.get_value("video", "max_fps", max_fps))
	fullscreen = bool(cf.get_value("video", "fullscreen", fullscreen))
	gui_scale = clampf(float(cf.get_value("hud", "gui_scale", gui_scale)), GUI_SCALES[0], GUI_SCALES[-1])
	net_address = String(cf.get_value("net", "address", net_address))
	net_name = String(cf.get_value("net", "name", net_name))
	jei_shown = bool(cf.get_value("hud", "jei_shown", jei_shown))
	jei_cols = clampi(int(cf.get_value("hud", "jei_cols", jei_cols)), 0, 16)
	mouse_sensitivity = float(cf.get_value("controls", "mouse_sensitivity", mouse_sensitivity))
	invert_y = bool(cf.get_value("controls", "invert_y", invert_y))
	brush_ghost = bool(cf.get_value("controls", "brush_ghost", brush_ghost))
	toggle_sprint = bool(cf.get_value("controls", "toggle_sprint", toggle_sprint))
	toggle_sneak = bool(cf.get_value("controls", "toggle_sneak", toggle_sneak))
	auto_step = bool(cf.get_value("controls", "auto_step", auto_step))
	hud_coords = bool(cf.get_value("hud", "coords", hud_coords))
	screen_effects = bool(cf.get_value("hud", "screen_effects", screen_effects))
	volume_master = clampf(float(cf.get_value("audio", "master", volume_master)), 0.0, 1.0)
	volume_music = clampf(float(cf.get_value("audio", "music", volume_music)), 0.0, 1.0)
	volume_effects = clampf(float(cf.get_value("audio", "effects", volume_effects)), 0.0, 1.0)
	volume_ui = clampf(float(cf.get_value("audio", "interface", volume_ui)), 0.0, 1.0)
	for a in REMAPPABLE:
		var v := String(cf.get_value("keys", a[0], ""))
		if v != "":
			bind_key(a[0], key_event(v))
	minimap_corner = String(cf.get_value("hud", "minimap_corner", minimap_corner))
	minimap_size_px = clampi(int(cf.get_value("hud", "minimap_size_px", minimap_size_px)), 0, 400)
	if not SHADOW_QUALITY.has(shadow_quality):
		shadow_quality = "high"
	graphics_preset = matching_preset()


func save_user_settings() -> void:
	var cf := ConfigFile.new()
	cf.set_value("video", "preset", graphics_preset)
	cf.set_value("video", "shadows", shadow_quality)
	cf.set_value("video", "ssao", ssao)
	cf.set_value("video", "bloom", bloom)
	cf.set_value("video", "fog", fog)
	cf.set_value("video", "brightness", brightness)
	cf.set_value("video", "view_distance_m", view_distance_m)
	cf.set_value("video", "fov", fov)
	cf.set_value("video", "vsync", vsync)
	cf.set_value("video", "max_fps", max_fps)
	cf.set_value("video", "fullscreen", fullscreen)
	cf.set_value("controls", "mouse_sensitivity", mouse_sensitivity)
	cf.set_value("controls", "invert_y", invert_y)
	cf.set_value("controls", "brush_ghost", brush_ghost)
	cf.set_value("controls", "toggle_sprint", toggle_sprint)
	cf.set_value("net", "address", net_address)
	cf.set_value("net", "name", net_name)
	cf.set_value("controls", "toggle_sneak", toggle_sneak)
	cf.set_value("controls", "auto_step", auto_step)
	cf.set_value("hud", "coords", hud_coords)
	cf.set_value("hud", "screen_effects", screen_effects)
	cf.set_value("audio", "master", volume_master)
	cf.set_value("audio", "music", volume_music)
	cf.set_value("audio", "effects", volume_effects)
	cf.set_value("audio", "interface", volume_ui)
	for a in REMAPPABLE:
		var ev := current_key(a[0])
		if ev != null and key_text(ev) != key_text(default_key(a[0])):
			cf.set_value("keys", a[0], key_string(ev))
	cf.set_value("hud", "gui_scale", gui_scale)
	cf.set_value("hud", "jei_shown", jei_shown)
	cf.set_value("hud", "jei_cols", jei_cols)
	cf.set_value("hud", "minimap_corner", minimap_corner)
	cf.set_value("hud", "minimap_size_px", minimap_size_px)
	cf.save(user_settings_path)


# ---------------- key remapping (plan item D11) ----------------

## The actions the Settings screen lets the player rebind, with their names there.
const REMAPPABLE := [["move_forward", "Forward"], ["move_back", "Back"], ["move_left", "Left"], ["move_right", "Right"],
	["jump", "Jump / fly up"], ["sprint", "Sprint"], ["sneak", "Sneak"], ["descend", "Fly down"],
	["action_break", "Break / attack"], ["action_place", "Use / place"], ["inventory", "Inventory"], ["drop", "Drop"],
	["swap_hands", "Swap hands"], ["map", "Map"], ["chat", "Chat"], ["zoom", "Zoom (hold)"]]


## The action's first event now (a key or a mouse button), or null.
func current_key(action: String) -> InputEvent:
	var evs := InputMap.action_get_events(action) if InputMap.has_action(action) else []
	return evs[0] if not evs.is_empty() else null


## The action's event in project.godot.
func default_key(action: String) -> InputEvent:
	var d = ProjectSettings.get_setting("input/" + action)
	if d is Dictionary and not (d["events"] as Array).is_empty():
		return d["events"][0]
	return null


## Makes `ev` the action's only event.
func bind_key(action: String, ev: InputEvent) -> void:
	if ev == null or not InputMap.has_action(action):
		return
	InputMap.action_erase_events(action)
	InputMap.action_add_event(action, ev)


func reset_keys() -> void:
	for a in REMAPPABLE:
		bind_key(a[0], default_key(a[0]))


## "key:<physical keycode>" or "mouse:<button>".
static func key_string(ev: InputEvent) -> String:
	if ev is InputEventMouseButton:
		return "mouse:%d" % ev.button_index
	if ev is InputEventKey:
		return "key:%d" % (ev.physical_keycode if ev.physical_keycode != 0 else ev.keycode)
	return ""


static func key_event(s: String) -> InputEvent:
	var kv := s.split(":")
	if kv.size() != 2 or not kv[1].is_valid_int():
		return null
	if kv[0] == "mouse":
		var m := InputEventMouseButton.new()
		m.button_index = int(kv[1]) as MouseButton
		m.pressed = true
		return m
	var k := InputEventKey.new()
	k.physical_keycode = int(kv[1]) as Key
	return k


## What the Settings screen shows for an event: "W", "Shift", "Left mouse".
static func key_text(ev: InputEvent) -> String:
	if ev is InputEventMouseButton:
		return {MOUSE_BUTTON_LEFT: "Left mouse", MOUSE_BUTTON_RIGHT: "Right mouse", MOUSE_BUTTON_MIDDLE: "Middle mouse"}.get(ev.button_index, "Mouse %d" % ev.button_index)
	if ev is InputEventKey:
		var code: int = ev.physical_keycode if ev.physical_keycode != 0 else ev.keycode
		return OS.get_keycode_string(DisplayServer.keyboard_get_keycode_from_physical(code) if ev.physical_keycode != 0 else code)
	return "-"


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
	get_window().content_scale_factor = gui_scale   # before the early outs: menu_test and captures scale too
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
