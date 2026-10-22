class_name Atmosphere
extends Node3D

## Everything about how the world is lit and what the air looks like: the
## WorldEnvironment (procedural sky shader, tonemap, bloom, SSAO, fog), the
## sun (shadows, colour, an optional day/night cycle), and two camera-driven
## states that read the world but never write it:
##   underground: the deeper the camera is below the generator's surface, the
##     darker the ambient and the darker/denser the fog (caves stop glowing sky blue)
##   submerged: the camera voxel is a liquid -> dense tinted fog + a screen tint
## Pure presentation; nothing here is saved or simulated. Tunables below;
## command line: time=0..1 (0.5 = noon), day_length=<seconds, 0 = frozen>,
## clouds=0..1, ssao=0|1, glow=0|1, fog=0|1, shadows=soft|hard|off, tonemap=aces|agx|filmic|linear,
## saturation=1.0. Measured cost of the defaults: ~1.2 ms/frame (see README).

# --- tunables ------------------------------------------------------------------
var time_of_day := 0.40          # 0 midnight, 0.25 sunrise, 0.5 noon, 0.75 sunset
var day_length_s := 0.0          # seconds per full day; 0 freezes the sun
var sun_yaw_deg := 35.0          # compass direction of the sun path (shadows never axis aligned)
var sun_energy := 1.25
var moon_energy := 0.10
var sun_color := Color(1.0, 0.96, 0.88)
var sunset_color := Color(1.0, 0.62, 0.35)
var moon_color := Color(0.62, 0.70, 0.95)
var shadow_distance_m := 96.0
var cloud_coverage := 0.42
var fog_density := 0.00035       # per world unit, above ground in daylight
var fog_color_day := Color(0.74, 0.83, 0.95)
var fog_color_night := Color(0.05, 0.06, 0.10)
var underground_start_m := 2.0   # metres below the surface where the cave look begins
var underground_full_m := 10.0   # ... and where it is complete
var underground_ambient := 0.30  # ambient energy multiplier fully underground (no torches exist yet)
var underground_fog_density := 0.0045
var underground_fog_color := Color(0.012, 0.012, 0.018)
var liquid_tints := {            # material name -> [fog colour, fog density, screen tint]
	"water": [Color(0.05, 0.22, 0.42), 0.09, Color(0.1, 0.3, 0.6, 0.28)],
	"water_static": [Color(0.05, 0.22, 0.42), 0.09, Color(0.1, 0.3, 0.6, 0.28)],
	"lava": [Color(0.9, 0.25, 0.02), 0.6, Color(1.0, 0.35, 0.05, 0.55)],
	"oil": [Color(0.06, 0.04, 0.02), 0.35, Color(0.15, 0.1, 0.03, 0.5)],
	"blood": [Color(0.3, 0.02, 0.02), 0.3, Color(0.55, 0.04, 0.04, 0.45)],
}
var ssao_on := true
var glow_on := true
var fog_on := true
var tonemap := "agx"             # aces | agx | filmic | linear
var shadows := "soft"            # soft (4 splits, penumbra) | hard (2 splits, no filter) | off
var saturation := 1.08

# --- nodes ---------------------------------------------------------------------
var env: Environment
var sun: DirectionalLight3D
var sky_material: ShaderMaterial
var overlay: ColorRect
var world: World
var camera: Camera3D

var _underground := 0.0          # smoothed 0..1
var _submerged_name := ""        # material name the camera is in, "" above
var _depth_check_accum := 0.0


func setup(p_world: World, p_camera: Camera3D) -> void:
	world = p_world
	camera = p_camera


func _ready() -> void:
	_apply_cmdline()
	var we := WorldEnvironment.new()
	we.name = "WorldEnvironment"
	env = Environment.new()
	env.background_mode = Environment.BG_SKY
	var sky := Sky.new()
	sky_material = ShaderMaterial.new()
	sky_material.shader = load("res://assets/shaders/sky.gdshader")
	sky_material.set_shader_parameter("cloud_coverage", cloud_coverage)
	sky_material.set_shader_parameter("sunset_color", Vector3(sunset_color.r, sunset_color.g, sunset_color.b))
	sky.sky_material = sky_material
	sky.radiance_size = Sky.RADIANCE_SIZE_128
	sky.process_mode = Sky.PROCESS_MODE_REALTIME   # the sky moves (clouds, sun); ambient follows
	env.sky = sky
	env.ambient_light_source = Environment.AMBIENT_SOURCE_SKY
	env.ambient_light_sky_contribution = 1.0
	env.ambient_light_energy = 1.0
	env.reflected_light_source = Environment.REFLECTION_SOURCE_SKY

	env.tonemap_mode = {"aces": Environment.TONE_MAPPER_ACES, "agx": Environment.TONE_MAPPER_AGX,
		"filmic": Environment.TONE_MAPPER_FILMIC, "linear": Environment.TONE_MAPPER_LINEAR}.get(tonemap, Environment.TONE_MAPPER_ACES)
	env.tonemap_exposure = 1.0
	env.tonemap_white = 6.0
	env.adjustment_enabled = true
	env.adjustment_saturation = saturation
	env.adjustment_contrast = 1.02

	# bloom only from genuinely bright things: lava, fire, glow caps, the sun's glint
	env.glow_enabled = glow_on
	env.glow_normalized = false
	env.glow_intensity = 0.55
	env.glow_strength = 1.0
	env.glow_bloom = 0.0
	env.glow_blend_mode = Environment.GLOW_BLEND_MODE_SOFTLIGHT
	env.glow_hdr_threshold = 1.5
	env.glow_hdr_scale = 2.0
	env.glow_hdr_luminance_cap = 12.0
	for i in 7:
		env.set_glow_level(i, 0.0)
	env.set_glow_level(1, 0.6)
	env.set_glow_level(2, 1.0)
	env.set_glow_level(3, 0.6)

	# screen-space AO: the crease between every voxel reads, plants sit on the ground
	env.ssao_enabled = ssao_on
	env.ssao_radius = 1.6            # world units (0.8 m)
	env.ssao_intensity = 2.2
	env.ssao_power = 1.6
	env.ssao_detail = 0.6
	env.ssao_horizon = 0.06
	env.ssao_sharpness = 0.98
	env.ssao_light_affect = 0.08
	env.ssao_ao_channel_affect = 0.0

	env.fog_enabled = fog_on
	env.fog_mode = Environment.FOG_MODE_EXPONENTIAL
	env.fog_light_color = fog_color_day
	env.fog_light_energy = 1.0
	env.fog_sun_scatter = 0.12
	env.fog_density = fog_density
	env.fog_aerial_perspective = 0.75    # far terrain fades into the sky's own colour, not a flat tint
	env.fog_sky_affect = 0.15            # the sky keeps its gradient near the horizon
	env.fog_height = Settings.SEA_LEVEL_VOXEL + 6.0
	env.fog_height_density = 0.0        # ground mist is set per frame (lowlands only, daylight)
	we.environment = env
	add_child(we)

	sun = DirectionalLight3D.new()
	sun.name = "Sun"
	sun.shadow_enabled = shadows != "off"
	sun.directional_shadow_mode = DirectionalLight3D.SHADOW_PARALLEL_4_SPLITS if shadows == "soft" else DirectionalLight3D.SHADOW_PARALLEL_2_SPLITS
	sun.directional_shadow_split_1 = 0.06
	sun.directional_shadow_split_2 = 0.18
	sun.directional_shadow_split_3 = 0.45
	sun.directional_shadow_blend_splits = true
	sun.directional_shadow_fade_start = 0.85
	sun.directional_shadow_max_distance = Settings.m2v(shadow_distance_m)
	sun.shadow_bias = 0.03
	sun.shadow_normal_bias = 0.7     # higher values leak light along voxel silhouettes
	sun.light_angular_distance = 0.6 if shadows == "soft" else 0.0   # penumbra (needs soft_shadow_filter_quality >= 1 in project settings)
	sun.sky_mode = DirectionalLight3D.SKY_MODE_LIGHT_AND_SKY
	add_child(sun)

	# full-screen tint when submerged (under the HUD, above the world)
	var layer := CanvasLayer.new()
	layer.layer = 4
	overlay = ColorRect.new()
	overlay.set_anchors_preset(Control.PRESET_FULL_RECT)
	overlay.mouse_filter = Control.MOUSE_FILTER_IGNORE
	overlay.color = Color(0, 0, 0, 0)
	layer.add_child(overlay)
	add_child(layer)
	_update_sun()
	_apply_states(true)


func _apply_cmdline() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"time": time_of_day = fposmod(float(kv[1]), 1.0)
			"day_length": day_length_s = float(kv[1])
			"clouds": cloud_coverage = clampf(float(kv[1]), 0.0, 1.0)
			"ssao": ssao_on = kv[1] == "1"
			"glow": glow_on = kv[1] == "1"
			"fog": fog_on = kv[1] == "1"
			"tonemap": tonemap = kv[1]
			"shadows": shadows = kv[1]
			"saturation": saturation = float(kv[1])
			"fxaa": get_viewport().screen_space_aa = Viewport.SCREEN_SPACE_AA_FXAA if kv[1] == "1" else Viewport.SCREEN_SPACE_AA_DISABLED


func _process(delta: float) -> void:
	if day_length_s > 0.0:
		time_of_day = fposmod(time_of_day + delta / day_length_s, 1.0)
		_update_sun()
	_depth_check_accum += delta
	if _depth_check_accum >= 0.1:   # 10 Hz is plenty for a fade; the lerp below smooths it
		_depth_check_accum = 0.0
		_sample_camera()
	_apply_states(false, delta)


## Sun elevation from the time of day: the light orbits around the X axis
## (tilted by sun_yaw), so one rotation is one day. Below the horizon the same
## light becomes the moon: dim, cool, from the opposite side.
func _update_sun() -> void:
	var angle := (time_of_day - 0.25) * TAU         # 0 at sunrise, PI/2 at noon
	var sun_up := sin(angle)
	var day := smoothstep(-0.05, 0.22, sun_up)
	var night := smoothstep(-0.05, 0.22, -sun_up)
	var is_moon := night > day
	var rot_x := -rad_to_deg(angle) + (180.0 if is_moon else 0.0)
	sun.rotation_degrees = Vector3(rot_x, sun_yaw_deg, 0)
	# the sky always wants the real sun, even while the light plays the moon. A
	# directional light shines along its -Z, so the direction towards the sun is +Z
	# of the un-flipped basis (-Z put the sun below the horizon: a night sky at noon).
	var sun_basis := Basis.from_euler(Vector3(-angle, deg_to_rad(sun_yaw_deg), 0))
	sky_material.set_shader_parameter("sun_dir", sun_basis.z)
	if is_moon:
		sun.light_color = moon_color
		sun.light_energy = moon_energy * night
	else:
		var low := 1.0 - smoothstep(0.0, 0.35, sun_up)
		sun.light_color = sun_color.lerp(sunset_color, low * 0.85)
		sun.light_energy = sun_energy * day
	sun.shadow_opacity = 0.9 if not is_moon else 0.6
	# the sky's fog colour follows daylight; the moon side stays dark blue
	var fog_c := fog_color_night.lerp(fog_color_day, day)
	var low2 := (1.0 - smoothstep(0.0, 0.3, absf(sun_up))) * smoothstep(-0.2, 0.0, sun_up)
	fog_c = fog_c.lerp(sunset_color, low2 * 0.35)
	set_meta("fog_day_color", fog_c)
	set_meta("daylight", day)


func _sample_camera() -> void:
	if world == null or camera == null:
		return
	var p := camera.global_position
	# submerged?
	var m := world.get_voxel(Vector3i(p.floor()))
	_submerged_name = ""
	if m > 0:
		var d: Dictionary = GameMaterials.defs()[m - 1]
		if liquid_tints.has(d["name"]):
			_submerged_name = d["name"]
	# below the surface?
	var surface := world.surface_height(int(floor(p.x)), int(floor(p.z)))
	var depth_m := Settings.v2m(float(surface) - p.y)
	set_meta("underground_target", clampf((depth_m - underground_start_m) / (underground_full_m - underground_start_m), 0.0, 1.0))


func _apply_states(instant: bool, delta: float = 0.0) -> void:
	var target: float = get_meta("underground_target", 0.0)
	if instant:
		_underground = target
	else:
		_underground = move_toward(_underground, target, delta * 2.5)
	var day: float = get_meta("daylight", 1.0)
	var fog_day: Color = get_meta("fog_day_color", fog_color_day)

	env.ambient_light_energy = lerpf(1.0, underground_ambient, _underground)
	env.ambient_light_sky_contribution = lerpf(1.0, 0.25, _underground)
	var tint := Color(0, 0, 0, 0)
	if _submerged_name != "":
		var t: Array = liquid_tints[_submerged_name]
		env.fog_light_color = t[0]
		env.fog_density = t[1]
		env.fog_aerial_perspective = 0.0
		env.fog_sky_affect = 1.0
		env.fog_height_density = 0.0
		tint = t[2]
	else:
		env.fog_light_color = fog_day.lerp(underground_fog_color, _underground)
		env.fog_density = lerpf(fog_density, underground_fog_density, _underground)
		env.fog_aerial_perspective = lerpf(0.75, 0.0, _underground)
		env.fog_sky_affect = lerpf(0.15, 0.0, _underground)
		# faint ground mist in the lowlands by day, none underground or at night
		env.fog_height_density = 0.0012 * day * (1.0 - _underground)
	overlay.color = tint
	# sun cannot reach deep caves beyond its shadow range: fade it with depth too
	sun.shadow_opacity = lerpf(0.9, 1.0, _underground)
