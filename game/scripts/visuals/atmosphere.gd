class_name Atmosphere
extends Node3D

## Everything about how the world is lit and what the air looks like: the
## WorldEnvironment (procedural sky shader, tonemap, bloom, SSAO, fog), the
## sun (shadows, colour, an optional day/night cycle), and two camera-driven
## states that read the world but never write it:
##   underground: the deeper the camera is below the generator's surface, the
##     darker the ambient and the darker/denser the fog (caves stop glowing sky blue)
##   submerged: the camera voxel is a liquid -> dense tinted fog + a screen tint
## The sun, the moon and the stars stand where `calendar` (scripts/world/calendar.gd,
## ADR 0025) puts them: real sun paths through the seasons, the moon's phase by the
## day of the month, sidereal stars, eclipses. The calendar is the only state here
## that is saved (by main.gd); the rest is presentation. Tunables below;
## command line: time=0..1 (0.5 = noon), date=<month>-<day> (1-based), day=<days since start>, latitude=<deg>,
## constellations=0|1, day_length=<seconds, 0 = frozen>,
## clouds=0..1, ssao=0|1, glow=0|1, fog=0|1, shadows=soft|hard|off, tonemap=aces|agx|filmic|linear,
## saturation=1.0, grade=golden|natural|clear (the lighting grade, ADR 0041). Measured cost of the defaults: ~1.2 ms/frame (see README).

# --- tunables ------------------------------------------------------------------
var calendar := Calendar.new()   # date, time, sun / moon / star geometry (saved by main.gd)
var time_of_day: float:          # 0 midnight, 0.5 noon (local solar time; sunrise moves with the seasons)
	get:
		return calendar.time
	set(v):
		calendar.time = fposmod(v, 1.0)
var day_length_s := 0.0          # seconds per full day; 0 freezes the sun
var sun_energy := 1.25
var moon_energy := 0.16          # a full moon high up; scaled by its phase and height
var show_constellations := false
var sun_radius := 0.042          # radians, as drawn (sky.gdshader)
var moon_radius := 0.042 * Calendar.MOON_SUN_RATIO   # mean size; the perigee cycle scales it
var sun_color := Color(1.0, 0.96, 0.88)
var sunset_color := Color(1.0, 0.62, 0.35)
var moon_color := Color(0.62, 0.70, 0.95)
var shadow_distance_m := 96.0
var cloud_coverage := 0.42
var overcast := 0.0              # Weather (ADR 0056): 0 clear .. 1 a heavy rain sky: clouds close, the sun dims, fog thickens
var fog_per_km := 6.0            # fog above ground in daylight, extinction per km (1 / e left after 1 / this km). 6: a hazy day, 30 % left at the 200 m horizon (ADR 0071)
var mist_per_km := 9.6           # the lowland ground mist by day (height fog), per km
var fog_color_day := Color(0.74, 0.83, 0.95)
var fog_color_night := Color(0.05, 0.06, 0.10)
var underground_start_m := 2.0   # metres below the surface where the cave look begins
var underground_full_m := 10.0   # ... and where it is complete
var sky_probe_m := 24.0          # how far up sky_open() follows the loaded voxels before it asks the generator
var underground_ambient := 0.30  # ambient energy multiplier fully underground (torches, lava and glow light the rest)
var underground_fog_density := 0.0045
var underground_fog_color := Color(0.012, 0.012, 0.018)
var underground_ambient_color := Color(0.5, 0.5, 0.56)  # neutral rock bounce, no sky tint (sRGB; x underground_ambient)
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
var shadow_splits := 4            # shadow_splits=2|4 (soft only): cascades
var shadow_blend := true          # shadow_blend=0|1: cross-fade between cascades
var shadow_filter := -1           # shadow_filter=0..5: directional soft-shadow filter quality (-1 = project setting)
var saturation := 1.08

# --- lighting grade (ADR 0041): one of GRADES sets everything below and the colours above ------
var grade := "golden"            # the developer's pick (2026-10-24)
var exposure := 1.0
var tonemap_white := 6.0
var contrast := 1.02
var curve: Array = []            # [[offset, Color], ...]: per-channel curves after the tonemap (split toning); [] = none
var ambient_mult := 1.0          # sky ambient above ground
var shadow_opacity := 0.9        # sun shadows by day (lower = lifted, softer shadows)
var aerial := 0.75               # how much far terrain takes the sky's colour
var sun_scatter := 0.12          # fog brightening towards the sun (haze)
var ssao_intensity := 2.2
var glow_intensity := 0.55
var glow_bloom := 0.0
var glow_threshold := 1.5
var zenith_color := Color(0.23, 0.45, 0.85)
var horizon_color := Color(0.72, 0.82, 0.95)

## The lighting grades of ADR 0041: golden is the game's look (the developer's pick, 2026-10-24), a warm
## afternoon sun, a warm haze and an S-curve like Lay of the Land. natural (the look before grades:
## neutral and hazy) and clear (thin air, a deep blue sky, crisp contrast) stay for comparison:
## `/grade <name>` or grade=<name>.
const GRADES := {
	"natural": {},
	"golden": {
		"tonemap": "aces", "exposure": 0.8, "tonemap_white": 8.0, "saturation": 1.2, "contrast": 1.08,
		"curve": [[0.0, Color(0.0, 0.01, 0.03)], [0.25, Color(0.22, 0.22, 0.24)], [0.5, Color(0.52, 0.5, 0.46)],
			[0.75, Color(0.79, 0.76, 0.70)], [1.0, Color(1.0, 0.97, 0.9)]],
		"sun_color": Color(1.0, 0.82, 0.6), "sunset_color": Color(1.0, 0.5, 0.22), "sun_energy": 1.6,
		"ambient_mult": 0.8, "shadow_opacity": 0.9, "fog_color_day": Color(0.85, 0.79, 0.69), "fog_per_km": 6.0,
		"aerial": 0.72, "sun_scatter": 0.35, "ssao_intensity": 2.0, "glow_intensity": 0.7, "glow_bloom": 0.03,
		"glow_threshold": 1.2, "zenith_color": Color(0.22, 0.42, 0.78), "horizon_color": Color(0.84, 0.79, 0.70),
	},
	"clear": {
		"tonemap": "agx", "exposure": 1.0, "tonemap_white": 6.0, "saturation": 1.22, "contrast": 1.1,
		"curve": [[0.0, Color(0.0, 0.0, 0.01)], [0.25, Color(0.21, 0.215, 0.23)], [0.5, Color(0.5, 0.5, 0.5)],
			[0.75, Color(0.79, 0.785, 0.77)], [1.0, Color(1.0, 1.0, 0.99)]],
		"sun_color": Color(1.0, 0.97, 0.92), "sunset_color": Color(1.0, 0.58, 0.3), "sun_energy": 1.4,
		"ambient_mult": 0.95, "shadow_opacity": 0.92, "fog_color_day": Color(0.62, 0.76, 0.95), "fog_per_km": 3.0,
		"aerial": 0.55, "sun_scatter": 0.08, "ssao_intensity": 2.2, "glow_intensity": 0.5, "glow_bloom": 0.0,
		"glow_threshold": 1.5, "zenith_color": Color(0.12, 0.36, 0.88), "horizon_color": Color(0.58, 0.74, 0.95),
	},
}
static var _natural := {}        # the defaults above, read once (natural = no overrides)

# --- nodes ---------------------------------------------------------------------
var env: Environment
var sun: DirectionalLight3D
var sky_material: ShaderMaterial
var overlay: ColorRect
var world: World
var camera: Camera3D

# season map: temperatures around the player for the terrain shader's snow and colours
const CLIMATE_N := 256                     # texels per side
const CLIMATE_STEP := int(16.0 / Settings.VOXEL_M)   # voxels per texel (16 m): 4 km across, the whole horizon
var _climate_task := -1
var _climate_bytes := PackedByteArray()
var _climate_rect := Vector4.ZERO
var _climate_tex: ImageTexture
var _climate_center := Vector2(INF, INF)
var _snow_t_max := 0.2

var _underground := 0.0          # smoothed 0..1
var _submerged_name := ""        # material name the camera is in, "" above
var _water_depth_m := 0.0        # metres of water above the camera while submerged in water
var _water_light := 1.0          # share of daylight that reaches that depth (eased)
var _sun_light := -1.0           # the sun / moon energy _update_sun chose, before water absorbs it
var water_light_m := 18.0        # light falls to 1/e every this many metres of water: dark below ~50 m
var _depth_check_accum := 0.0


func setup(p_world: World, p_camera: Camera3D) -> void:
	world = p_world
	_push_grade()   # the terrain materials take the sky's colours too (ADR 0073)
	camera = p_camera
	_apply_season()   # _ready ran before the world was known


func _ready() -> void:
	_read_user_settings()
	for a in OS.get_cmdline_user_args():   # the grade first: single values on the command line override it
		if a.begins_with("grade="):
			grade = a.substr(6)
	_load_grade(grade)
	_apply_cmdline()
	var we := WorldEnvironment.new()
	we.name = "WorldEnvironment"
	env = Environment.new()
	env.background_mode = Environment.BG_SKY
	var sky := Sky.new()
	sky_material = ShaderMaterial.new()
	sky_material.shader = load("res://assets/shaders/sky.gdshader")
	sky_material.set_shader_parameter("cloud_coverage", cloud_coverage)
	sky_material.set_shader_parameter("star_map", load("res://assets/textures/sky_stars.png"))
	sky_material.set_shader_parameter("milky_way", load("res://assets/textures/sky_milkyway.png"))
	sky_material.set_shader_parameter("sun_radius", sun_radius)
	sky_material.set_shader_parameter("moon_radius", moon_radius)
	sky.sky_material = sky_material
	sky.radiance_size = Sky.RADIANCE_SIZE_128
	sky.process_mode = Sky.PROCESS_MODE_REALTIME   # the sky moves (clouds, sun); ambient follows
	env.sky = sky
	env.ambient_light_source = Environment.AMBIENT_SOURCE_SKY
	env.ambient_light_sky_contribution = 1.0
	env.ambient_light_energy = 1.0
	env.reflected_light_source = Environment.REFLECTION_SOURCE_SKY
	env.adjustment_enabled = true

	# bloom only from genuinely bright things: lava, fire, glow caps, the sun's glint
	env.glow_enabled = glow_on
	env.glow_normalized = false
	env.glow_strength = 1.0
	env.glow_blend_mode = Environment.GLOW_BLEND_MODE_SOFTLIGHT
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
	env.fog_density = fog_per_km / Settings.m2v(1000.0)
	env.fog_sky_affect = 0.15            # the sky keeps its gradient near the horizon
	env.fog_height = Settings.SEA_LEVEL_VOXEL + 6.0
	env.fog_height_density = 0.0        # ground mist is set per frame (lowlands only, daylight)
	_push_grade()
	we.environment = env
	add_child(we)

	sun = DirectionalLight3D.new()
	sun.name = "Sun"
	_apply_shadows()
	sun.directional_shadow_split_1 = 0.06
	sun.directional_shadow_split_2 = 0.18
	sun.directional_shadow_split_3 = 0.45
	sun.directional_shadow_fade_start = 0.85
	sun.shadow_bias = 0.03
	sun.shadow_normal_bias = 0.7     # higher values leak light along voxel silhouettes
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
	var temp_bands: Array = BiomeRegistry.load_default().climate.get("temperature_bands", [])
	for b in temp_bands:
		if b.get("name", "") == "temperate":
			_snow_t_max = float(b["max"])   # snow settles in temperate and colder climates
	_update_sun()
	_apply_states(true)


func _exit_tree() -> void:
	if _climate_task >= 0:
		WorkerThreadPool.wait_for_task_completion(_climate_task)
		_climate_task = -1

## /time set and sleeping: move forward to the next time `t` (0 midnight, 0.5 noon);
## the date moves on with it, never back.
func set_time_of_day(t: float) -> void:
	calendar.advance_to(t)
	_update_sun()


func set_show_constellations(on: bool) -> void:
	show_constellations = on
	_update_sun()


## /grade: switch the lighting grade live. False for an unknown name.
func apply_grade(p_name: String) -> bool:
	if not _load_grade(p_name):
		return false
	_push_grade()
	_update_sun()
	_apply_states(true)
	return true


## Sets this node's values from a grade (keys it leaves out keep natural's).
func _load_grade(p_name: String) -> bool:
	if not GRADES.has(p_name):
		return false
	if _natural.is_empty():   # first call: the defaults are still untouched
		for g: Dictionary in GRADES.values():
			for k in g:
				_natural[k] = get(k)
	grade = p_name
	var g: Dictionary = GRADES[p_name]
	for k in _natural:
		set(k, g.get(k, _natural[k]))
	return true


## The grade's values on the environment and the sky (the sun and fog colours follow in _update_sun).
func _push_grade() -> void:
	env.tonemap_mode = {"aces": Environment.TONE_MAPPER_ACES, "agx": Environment.TONE_MAPPER_AGX,
		"filmic": Environment.TONE_MAPPER_FILMIC, "linear": Environment.TONE_MAPPER_LINEAR}.get(tonemap, Environment.TONE_MAPPER_ACES)
	env.tonemap_exposure = exposure
	env.tonemap_white = tonemap_white
	env.adjustment_saturation = saturation
	env.adjustment_contrast = contrast
	env.adjustment_color_correction = _curve_texture(curve)
	env.glow_intensity = glow_intensity
	env.glow_bloom = glow_bloom
	env.glow_hdr_threshold = glow_threshold
	env.ssao_intensity = ssao_intensity
	env.fog_sun_scatter = sun_scatter
	env.fog_aerial_perspective = aerial
	_set_sky("sunset_color", Vector3(sunset_color.r, sunset_color.g, sunset_color.b))
	_set_sky("zenith_color", Vector3(zenith_color.r, zenith_color.g, zenith_color.b))
	_set_sky("horizon_color", Vector3(horizon_color.r, horizon_color.g, horizon_color.b))
	# below the horizon the sky is the haze the terrain blends into (ADR 0073), not a grey ground
	_set_sky("ground_color", Vector3(fog_color_day.r, fog_color_day.g, fog_color_day.b))


## A sky uniform shared with the terrain materials, which blend into the sky's own colour at the
## horizon (sky_common.gdshaderinc, ADR 0073).
func _set_sky(param: String, value: Variant) -> void:
	sky_material.set_shader_parameter(param, value)
	if world:
		for m in world.horizon_materials():
			m.set_shader_parameter(param, value)


## A 1D lookup: each channel is remapped through its own curve (Godot reads r from .r, g from .g ...).
static func _curve_texture(points: Array) -> GradientTexture1D:
	if points.is_empty():
		return null
	var g := Gradient.new()
	g.offsets = PackedFloat32Array(points.map(func(p: Array) -> float: return p[0]))
	g.colors = PackedColorArray(points.map(func(p: Array) -> Color: return p[1]))
	var t := GradientTexture1D.new()
	t.gradient = g
	t.width = 256
	return t


## The player's graphics options (Settings, ADR 0032) as this node's values;
## command-line args (ssao=, shadows=, shadow_splits= ...) override them after.
func _read_user_settings() -> void:
	ssao_on = Settings.ssao
	glow_on = Settings.bloom
	var q: Dictionary = Settings.SHADOW_QUALITY.get(Settings.shadow_quality, Settings.SHADOW_QUALITY["high"])
	shadows = q["mode"]
	shadow_splits = int(q["splits"])
	shadow_filter = int(q["filter"])
	if float(q["distance_m"]) > 0.0:
		shadow_distance_m = float(q["distance_m"])


func _apply_shadows() -> void:
	sun.shadow_enabled = shadows != "off"
	sun.directional_shadow_mode = DirectionalLight3D.SHADOW_PARALLEL_4_SPLITS if shadows == "soft" and shadow_splits == 4 else DirectionalLight3D.SHADOW_PARALLEL_2_SPLITS
	sun.directional_shadow_blend_splits = shadow_blend
	if shadow_filter >= 0:
		RenderingServer.directional_soft_shadow_filter_set_quality(shadow_filter)
	sun.directional_shadow_max_distance = Settings.m2v(shadow_distance_m)
	sun.light_angular_distance = 0.6 if shadows == "soft" else 0.0   # penumbra (needs soft_shadow_filter_quality >= 1 in project settings)


## The Settings screen changed something: shadows, ambient occlusion and bloom, live.
func apply_graphics() -> void:
	_read_user_settings()
	env.ssao_enabled = ssao_on
	env.glow_enabled = glow_on
	_apply_shadows()


func _apply_cmdline() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"time": time_of_day = fposmod(float(kv[1]), 1.0)
			"date":
				var md := kv[1].split("-")
				if md.size() == 2:
					calendar.set_date(int(md[0]) - 1, int(md[1]) - 1)
			"latitude": calendar.latitude_deg = clampf(float(kv[1]), -89.0, 89.0)
			"day": calendar.day = maxi(0, int(kv[1]))   # whole days since the world began (eclipse shots)
			"constellations": show_constellations = kv[1] == "1"
			"day_length": day_length_s = float(kv[1])
			"clouds": cloud_coverage = clampf(float(kv[1]), 0.0, 1.0)
			"ssao": ssao_on = kv[1] == "1"
			"glow": glow_on = kv[1] == "1"
			"fog": fog_on = kv[1] == "1"
			"tonemap": tonemap = kv[1]
			"shadows": shadows = kv[1]
			"shadow_splits": shadow_splits = int(kv[1])
			"shadow_blend": shadow_blend = kv[1] == "1"
			"shadow_filter": shadow_filter = int(kv[1])
			"shadow_distance_m": shadow_distance_m = float(kv[1])
			"saturation": saturation = float(kv[1])
			"fxaa": get_viewport().screen_space_aa = Viewport.SCREEN_SPACE_AA_FXAA if kv[1] == "1" else Viewport.SCREEN_SPACE_AA_DISABLED


func _process(delta: float) -> void:
	if day_length_s > 0.0:
		calendar.advance(delta / day_length_s)
		_update_sun()
	_depth_check_accum += delta
	if _depth_check_accum >= 0.1:   # 10 Hz is plenty for a fade; the lerp below smooths it
		_depth_check_accum = 0.0
		_sample_camera()
		_update_climate()
	_apply_states(false, delta)


## Sun, moon and sky from the calendar. One DirectionalLight plays the sun by day
## and the moon by night (its energy follows the phase and the moon's height);
## the sky shader draws both discs, the stars turned by sidereal time, and eclipses.
func _update_sun() -> void:
	var sd := calendar.sun_dir()
	var md := calendar.moon_dir()
	var size := calendar.moon_size()
	var e := calendar.eclipse()
	var basis := calendar.celestial_to_world()
	var pole := basis * Vector3(0, 0, 1)
	var sun_up := sd.y
	var solar := 0.0
	var cover := 0.0
	var lunar_cover := 0.0
	var shadow := Vector2(9.0, 0.0)
	var moon_draw := md
	var mr := moon_radius * size
	if e["kind"] == "solar":
		# the drawn moon crosses the drawn sun at the real eclipse's closest approach
		var east := pole.cross(sd).normalized()
		var north := sd.cross(east)
		var r_sum := sun_radius + mr
		var along: float = float(e["progress"]) * r_sum * 1.6
		var miss: float = float(e["miss"]) * r_sum
		moon_draw = (sd + east * along + north * miss).normalized()
		solar = 1.0
		cover = disc_overlap(sqrt(along * along + miss * miss), sun_radius, mr)
	elif e["kind"] == "lunar":
		# the umbra crosses the moon from east to west (in moon radii, disc frame x west)
		var travel := 2.0 * (2.7 + 1.0)
		shadow = Vector2(float(e["progress"]) * travel * 0.5, float(e["miss"]) * 3.7)
		lunar_cover = disc_overlap(shadow.length(), 2.7, 1.0)
	var day := smoothstep(-0.05, 0.22, sun_up) * (1.0 - 0.95 * cover * cover)
	var night := smoothstep(-0.05, 0.22, -sun_up)
	var moon_light := moon_energy * calendar.moon_illumination() * smoothstep(-0.02, 0.2, md.y) * night * (1.0 - 0.9 * lunar_cover)
	var sun_light := sun_energy * day
	var is_moon := moon_light > sun_light
	var light_dir := md if is_moon else sd
	var up := Vector3.UP if absf(light_dir.y) < 0.98 else Vector3.FORWARD
	sun.basis = Basis.looking_at(-light_dir, up)   # a directional light shines along its -Z
	if is_moon:
		sun.light_color = moon_color
		_sun_light = moon_light
	else:
		var low := 1.0 - smoothstep(0.0, 0.35, sun_up)
		sun.light_color = sun_color.lerp(sunset_color, low * 0.85)
		_sun_light = sun_light
	sun.light_energy = _sun_light * _water_light
	sun.shadow_opacity = shadow_opacity if not is_moon else 0.6
	_set_sky("sun_dir", sd)
	sky_material.set_shader_parameter("moon_dir", moon_draw)
	sky_material.set_shader_parameter("moon_size", size)
	sky_material.set_shader_parameter("pole_dir", pole)
	sky_material.set_shader_parameter("world_to_sky", basis.transposed())
	sky_material.set_shader_parameter("solar_eclipse", solar)
	_set_sky("eclipse_dark", cover)
	sky_material.set_shader_parameter("lunar_shadow", shadow)
	sky_material.set_shader_parameter("constellation_lines", 1.0 if show_constellations else 0.0)
	# the autumn moons rise orange; the rest are a cool white
	var tint := Color(1.0, 0.98, 0.94)
	if calendar.moon_illumination() > 0.8 and calendar.month() in [8, 9]:
		tint = Color(1.0, 0.86, 0.66)
	sky_material.set_shader_parameter("moon_tint", Vector3(tint.r, tint.g, tint.b))
	# the sky's fog colour follows daylight; the moon side stays dark blue
	var fog_c := fog_color_night.lerp(fog_color_day, day)
	var low2 := (1.0 - smoothstep(0.0, 0.3, absf(sun_up))) * smoothstep(-0.2, 0.0, sun_up)
	fog_c = fog_c.lerp(sunset_color, low2 * 0.35)
	set_meta("fog_day_color", fog_c)
	set_meta("daylight", day)
	_apply_season()


## Season looks on the terrain material (autumn leaves, winter grass, snow cover).
func _apply_season() -> void:
	if world == null or world.atlas_material == null:
		return
	var m := world.atlas_material
	m.set_shader_parameter("season_autumn", calendar.autumn_colour())
	m.set_shader_parameter("season_winter", calendar.winter_dullness())
	m.set_shader_parameter("season_snow", calendar.snow_cover())
	m.set_shader_parameter("snow_t_max", _snow_t_max)


## Rebuilds the climate map on a worker when the camera has moved 512 m from its
## centre (65k native column samples, ~0.2 s off the main thread).
func _update_climate() -> void:
	if world == null or camera == null or world.generator == null or world.atlas_material == null:
		return
	if _climate_task >= 0:
		if not WorkerThreadPool.is_task_completed(_climate_task):
			return
		WorkerThreadPool.wait_for_task_completion(_climate_task)
		_climate_task = -1
		if _climate_bytes.size() != CLIMATE_N * CLIMATE_N:
			_climate_center = Vector2(INF, INF)   # try again next time
			return
		var img := Image.create_from_data(CLIMATE_N, CLIMATE_N, false, Image.FORMAT_L8, _climate_bytes)
		# one texture, updated in place: a new texture on the terrain material makes the render
		# thread rebuild every terrain block's uniform set (a 15 ms frame, ADR 0033)
		if _climate_tex == null:
			_climate_tex = ImageTexture.create_from_image(img)
			world.atlas_material.set_shader_parameter("climate_map", _climate_tex)
		else:
			_climate_tex.update(img)
		world.atlas_material.set_shader_parameter("climate_rect", _climate_rect)
		return
	var p := Vector2(camera.global_position.x, camera.global_position.z)
	if p.distance_to(_climate_center) < Settings.m2v(512.0):
		return
	_climate_center = p
	var span := CLIMATE_N * CLIMATE_STEP
	var x0 := int(p.x) - span / 2
	var z0 := int(p.y) - span / 2
	# texel i is sampled at x0 + i * step: shift the rect half a texel so they line up
	_climate_rect = Vector4(x0 - CLIMATE_STEP * 0.5, z0 - CLIMATE_STEP * 0.5, span, span)
	var gen: FineGen = world.generator.gen
	_climate_task = WorkerThreadPool.add_task(func() -> void:
		_climate_bytes = gen.climate_map(x0, z0, CLIMATE_N, CLIMATE_STEP))


## Fraction of disc A (radius ra) covered by disc B (radius rb) at centre distance d.
static func disc_overlap(d: float, ra: float, rb: float) -> float:
	if d >= ra + rb:
		return 0.0
	if d <= absf(rb - ra):
		return 1.0 if rb >= ra else (rb * rb) / (ra * ra)
	var a := ra * ra * acos(clampf((d * d + ra * ra - rb * rb) / (2.0 * d * ra), -1.0, 1.0))
	var b := rb * rb * acos(clampf((d * d + rb * rb - ra * ra) / (2.0 * d * rb), -1.0, 1.0))
	var c := 0.5 * sqrt(maxf(0.0, (-d + ra + rb) * (d + ra - rb) * (d - ra + rb) * (d + ra + rb)))
	return clampf((a + b - c) / (PI * ra * ra), 0.0, 1.0)


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
	# how deep under water: up to 24 voxels by reading, deeper in the sea by sea level
	_water_depth_m = 0.0
	if _submerged_name.begins_with("water"):
		var v := Vector3i(p.floor())
		var n := 0
		while n < 24 and world.get_voxel(v + Vector3i(0, n + 1, 0)) in [GameMaterials.WATER, GameMaterials.WATER_STATIC]:
			n += 1
		var depth_v := float(n) + (float(v.y) + 1.0 - p.y)
		if n >= 24:
			depth_v = maxf(depth_v, float(Settings.SEA_LEVEL_VOXEL) - p.y)
		_water_depth_m = Settings.v2m(depth_v)
	# below the surface?
	var surface := world.surface_height(int(floor(p.x)), int(floor(p.z)))
	var depth_m := Settings.v2m(float(surface) - p.y)
	var target := clampf((depth_m - underground_start_m) / (underground_full_m - underground_start_m), 0.0, 1.0)
	# the bottom of a ravine or a mouth is below the surface height but under the open sky
	if target > 0.0 and sky_open(p, surface):
		target = 0.0
	set_meta("underground_target", target)


## True when nothing solid stands between `p` and the sky: a vertical ray through the loaded voxels
## (sky_probe_m, so dug shafts and built roofs count), and past them the generator's cave map (does
## a cave break through the ground in this column: a ravine or a cave mouth).
func sky_open(p: Vector3, surface: int) -> bool:
	var depth_v := float(surface) - p.y
	if depth_v <= 0.0:
		return true
	var reach := minf(depth_v + 1.0, Settings.m2v(sky_probe_m))
	if world.raycast(p, Vector3.UP, reach) != null:
		return false
	if reach > depth_v:
		return true
	var gen = world.generator.gen if world.generator != null else null
	if gen == null:
		return false
	var cm: PackedByteArray = gen.cave_map(int(floor(p.x)), int(floor(p.z)), 1, 1)
	return cm.size() >= 2 and cm[1] == 1


var _overcast_shown := 0.0

func _apply_states(instant: bool, delta: float = 0.0) -> void:
	var target: float = get_meta("underground_target", 0.0)
	if instant:
		_underground = target
	else:
		_underground = move_toward(_underground, target, delta * 2.5)
	var day: float = get_meta("daylight", 1.0)
	var fog_day: Color = get_meta("fog_day_color", fog_color_day)

	env.ambient_light_energy = lerpf(ambient_mult, underground_ambient, _underground)
	# underground the ambient is a dim neutral bounce, not a share of the (golden, sunlit) sky
	env.ambient_light_color = underground_ambient_color
	env.ambient_light_sky_contribution = 1.0 - _underground
	var tint := Color(0, 0, 0, 0)
	# water swallows light with depth: the sea floor at 100 m is close to black (2026-10-23)
	var light_target := exp(-_water_depth_m / water_light_m) if _submerged_name.begins_with("water") else 1.0
	_water_light = light_target if instant else lerpf(_water_light, light_target, clampf(delta * 3.0, 0.0, 1.0))
	if _sun_light >= 0.0:
		# no sun in deep caves (bug 2026-10-25_21-21-36): the shadow map ends at shadow_distance_m and its
		# coarse cascades leak, so hall walls past it were sunlit through 80 m of rock
		sun.light_energy = _sun_light * _water_light * lerpf(1.0, 0.3, overcast) * (1.0 - _underground)
	if overcast > 0.0 or _overcast_shown > 0.0:
		_overcast_shown = overcast
		sky_material.set_shader_parameter("cloud_coverage", lerpf(cloud_coverage, 1.0, overcast))
	env.ambient_light_energy *= lerpf(1.0, 0.75, overcast)
	# no glow towards a sun the cave hides (the orange haze of bug 2026-10-25_21-21-36)
	env.fog_sun_scatter = sun_scatter * (1.0 - _underground)
	if _submerged_name != "":
		var t: Array = liquid_tints[_submerged_name]
		var dark := 1.0 - _water_light
		var lit := lerpf(0.25, 1.0, day) * lerpf(0.06, 1.0, _water_light)   # night water is dark too
		var fc: Color = t[0]
		env.fog_light_color = Color(fc.r * lit, fc.g * lit, fc.b * lit)
		env.fog_density = lerpf(float(t[1]), float(t[1]) * 1.6, dark)
		env.fog_aerial_perspective = 0.0
		env.fog_sky_affect = 1.0
		env.fog_height_density = 0.0
		env.ambient_light_energy *= lerpf(1.0, 0.08, dark)
		var tc: Color = t[2]
		tint = Color(tc.r * lit, tc.g * lit, tc.b * lit, lerpf(tc.a, 0.5, dark))
	else:
		env.fog_light_color = fog_day.lerp(underground_fog_color, _underground)
		env.fog_density = lerpf(fog_per_km * (1.0 + 4.0 * overcast) / Settings.m2v(1000.0), underground_fog_density, _underground)
		env.fog_aerial_perspective = lerpf(aerial, 0.0, _underground)
		# underground the fog hides everything far, the sky too: a hole in the far LOD meshes shows
		# cave fog, not a bright sky (bug 2026-10-25_21-21-14)
		env.fog_sky_affect = lerpf(0.15, 1.0, _underground)
		# faint ground mist in the lowlands by day, none underground or at night
		env.fog_height_density = mist_per_km / Settings.m2v(1000.0) * day * (1.0 - _underground)
	overlay.color = tint
	# sun cannot reach deep caves beyond its shadow range: fade it with depth too
	sun.shadow_opacity = lerpf(shadow_opacity, 1.0, _underground)
