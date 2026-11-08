class_name Weather
extends Node3D

## Weather (ADR 0056): clear, rain or snow over the player, and the answer to forest fires.
## - Random weather: every Settings.weather_roll_s a roll by the column's humidity (dry biomes
##   rarely, wet ones often) may start a shower of weather_rain_s; below 0 C it snows.
## - Fires: sim cells are sampled around the player (SimBubble.sample_cells, like the sound loops):
##   flames, smoke and burning cells. The first fire seen starts a timer of fire_rain_delay_s; when it
##   runs out it rains until no fire has been seen for weather_fire_clear_s. A region burns, not the map.
## - While it rains, single water cells are dropped on the fires the samples found, into the air (or
##   smoke, flames) just above each one. Water puts a burning cell out (SimWorld: a burning cell next to
##   a non-burnable liquid stops burning); what is left runs off into puddles: wet ground. Not
##   SimBubble.blast: a sphere edit runs on the worker pool, which a burning forest keeps busy with
##   rigid piece checks, and it blocked 200-300 ms (2026-10-25 fire run).
## - What the player sees and hears: Atmosphere.overcast (clouds, dim sun, fog), rain or snow
##   particles that a heightfield collision stops on roofs and canopies, a rain loop, a wet coat.
## GDScript only orchestrates: a few hundred sampled cells and drops a second (principle 8).
## Flint and steel lights fires through ignite().
## Storms (ADR 0149): a shower in a wet column may be a storm; then every lightning_every_s or so a strike,
## half of them distant (a dim flash and far thunder: the warning), the rest on the highest surface voxel
## around the player (SimWorld.surface_map every 0.75 m: a tree top, a tower, a roof): a flash, thunder delayed by distance, flames in
## the air there and the hit voxel alight if it burns, damage to a player standing at the spot. A lightning
## rod (LightningRodEntity) within lightning_rod_m of the aim draws the strike to its tip and grounds it.

signal changed(kind: String)

const FIRE := 12    # GameMaterials "fire" (flames)
const SMOKE := 11

var main: Node
var kind := "clear"          # "clear", "rain" (snow when the air is below 0 C), "storm" (rain with lightning)
var intensity := 0.0         # shown: eases toward target
var target := 0.0
var _until := 0.0            # play seconds when the current shower ends (0: open-ended, the fire rain)
var _clock := 0.0
var _roll_t := 0.0
var _scan_t := 0.0
var _drop_t := 0.0
var _rng := RandomNumberGenerator.new()
var _mask := PackedByteArray()
var _strike_t := 0.0
var _metal := PackedByteArray()   # materials that draw lightning (metal_mask)
var _air_like := PackedInt32Array()   # what a rain drop may replace besides air
var _fires: Array[Vector3i] = []   # recent fire sample positions (newest last): burning cells and flames, smoke moved down
var _fire_seen := -1e9             # _clock when fire was last seen
var fire_rain_at := -1.0           # _clock when the fire rain starts, -1 none scheduled
var _fire_rain := false            # the current rain answers a fire
var _seed := 0
var _rain: GPUParticles3D
var _snow: GPUParticles3D
var _collide: GPUParticlesCollisionHeightField3D
var _sound: AudioStreamPlayer
var _wet_t := 0.0
var _hits := 0               # fire cells the last scan found
var _history: Array = []     # [s, fire cells found, intensity] every 5 s (scripted checks read it in the JSON)
var _warm_t := 1.5           # both particle systems emit a trace under the loading screen: their pipelines compile there
var stats := {"fires_seen": 0, "fire_rains": 0, "showers": 0, "drops": 0, "scan_ms_max": 0.0, "drop_ms_max": 0.0, "ignited": 0, "strikes": 0, "far_strikes": 0, "grounded": 0, "last_strike": []}


func setup(p_main: Node) -> void:
	main = p_main
	_rng.seed = 777
	_mask.resize(256)
	_mask[FIRE] = 1
	_mask[SMOKE] = 1
	_metal = metal_mask()
	_air_like = PackedInt32Array([SMOKE, FIRE, GameMaterials.id_of("steam")])
	_roll_t = _rng.randf_range(0.3, 1.0) * Settings.weather_roll_s
	_rain = _make_particles(false)
	_snow = _make_particles(true)
	_collide = GPUParticlesCollisionHeightField3D.new()
	_collide.name = "RainRoofs"
	_collide.size = Vector3(Settings.m2v(40.0), Settings.m2v(48.0), Settings.m2v(40.0))
	_collide.resolution = GPUParticlesCollisionHeightField3D.RESOLUTION_256
	_collide.update_mode = GPUParticlesCollisionHeightField3D.UPDATE_MODE_WHEN_MOVED
	_collide.follow_camera_enabled = true
	add_child(_collide)
	_sound = AudioStreamPlayer.new()
	_sound.name = "RainLoop"
	_sound.bus = "World"
	_sound.volume_db = -80.0
	add_child(_sound)
	var s: Dictionary = Sound._sounds.get("loop/rain", {})   # like GameAudio._make_loop: one looping voice
	if not s.is_empty():
		var st := Sound._stream(s["files"][0])
		if st is AudioStreamOggVorbis:
			(st as AudioStreamOggVorbis).loop = true
		_sound.stream = st
	for a in OS.get_cmdline_user_args():
		if a.begins_with("weather="):
			set_weather(a.get_slice("=", 1), 0.0)


## Particles drawn under the loading screen once (their first draw compiles the pipeline).
func warm_materials() -> Array:
	return [_rain.draw_pass_1.surface_get_material(0), _snow.draw_pass_1.surface_get_material(0)]


func _make_particles(snow: bool) -> GPUParticles3D:
	var p := GPUParticles3D.new()
	p.name = "Snow" if snow else "Rain"
	p.amount = Settings.weather_particles
	p.lifetime = 4.0 if snow else 1.2
	p.preprocess = 1.0
	p.local_coords = false
	p.emitting = false
	p.visibility_aabb = AABB(Vector3(-1, -1, -1) * Settings.m2v(30.0), Vector3.ONE * Settings.m2v(60.0))
	p.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	var pm := ParticleProcessMaterial.new()
	pm.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_BOX
	pm.emission_box_extents = Vector3(Settings.m2v(16.0), Settings.m2v(1.0), Settings.m2v(16.0))
	pm.direction = Vector3(0.08, -1, 0.03)
	pm.spread = 3.0 if not snow else 25.0
	var v := Settings.m2v(1.6 if snow else 11.0)
	pm.initial_velocity_min = v * 0.85
	pm.initial_velocity_max = v
	pm.gravity = Vector3.ZERO
	pm.turbulence_enabled = snow
	pm.collision_mode = ParticleProcessMaterial.COLLISION_HIDE_ON_CONTACT
	p.process_material = pm
	var q := QuadMesh.new()
	q.size = Vector2(Settings.m2v(0.04), Settings.m2v(0.04)) if snow else Vector2(Settings.m2v(0.012), Settings.m2v(0.5))
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	m.billboard_mode = BaseMaterial3D.BILLBOARD_FIXED_Y if not snow else BaseMaterial3D.BILLBOARD_ENABLED
	m.billboard_keep_scale = true
	m.albedo_color = Color(0.95, 0.97, 1.0, 0.9) if snow else Color(0.7, 0.78, 0.9, 0.35)
	q.material = m
	p.draw_pass_1 = q
	add_child(p)
	return p


# ---------------- control ----------------

## /weather: "clear", "rain" (snow when cold) for `seconds` (0: until the next roll).
func set_weather(k: String, seconds: float) -> bool:
	match k:
		"clear":
			_stop()
			return true
		"rain", "snow", "storm":
			_start(seconds if seconds > 0.0 else Settings.weather_rain_s, false, k == "storm")
			return true
	return false


func _start(seconds: float, for_fire: bool, storm := false) -> void:
	kind = "storm" if storm else "rain"
	_strike_t = 3.0
	target = 1.0
	_fire_rain = for_fire
	_until = 0.0 if for_fire else _clock + seconds
	fire_rain_at = -1.0
	stats["fire_rains" if for_fire else "showers"] += 1
	changed.emit(kind)


func _stop() -> void:
	kind = "clear"
	target = 0.0
	_fire_rain = false
	_until = 0.0
	changed.emit(kind)


func is_raining() -> bool:
	return kind != "clear"


## The fire rain timer in seconds, or -1.
func fire_rain_in() -> float:
	return fire_rain_at - _clock if fire_rain_at >= 0.0 else -1.0


## Flint and steel (ADR 0056): flames on the clicked face, against what was aimed at. False when the
## spot is not air (or a plant that burns) inside the simulated area.
func ignite(hit: VoxelRaycastResult) -> bool:
	if hit == null or main.bubble == null:
		return false
	var spot: Vector3i = hit.previous_position
	var here: int = main.bubble.get_voxel(spot)
	if here < 0:
		return false
	if here != GameMaterials.AIR:
		var d: Dictionary = GameMaterials.defs()[here - 1] if here <= GameMaterials.defs().size() else {}
		if not d.get("burnable", false):
			return false
	var was := Net.recording
	Net.recording = false   # a flame is not an edit: the journal would light it again at every replay
	var lit: bool = main.bubble.set_voxel(spot, FIRE)
	Net.recording = was
	if not lit:
		return false
	Net.send_live(NetOps.voxels(PackedInt32Array([spot.x, spot.y, spot.z]), PackedByteArray([FIRE])))   # ADR 0165: lit for everyone, never replayed
	stats["ignited"] += 1
	fire_started(Vector3(spot))
	Sound.play("sim/ignite", Vector3(spot) + Vector3(0.5, 0.5, 0.5))
	return true


## A fire another player lit (ADR 0165): heard and tracked like one lit here.
func fire_heard(spot: Vector3) -> void:
	stats["heard"] = int(stats.get("heard", 0)) + 1
	fire_started(spot)
	Sound.play("sim/ignite", spot + Vector3(0.5, 0.5, 0.5))


## A fire somewhere (flint and steel, a burst charcoal pit): starts the rain timer at once.
func fire_started(at: Vector3) -> void:
	_note_fire(Vector3i(at.floor()))


func _note_fire(p: Vector3i) -> void:
	if _clock - _fire_seen > Settings.weather_fire_clear_s:
		stats["fires_seen"] += 1
	_fire_seen = _clock
	_fires.append(p)
	if _fires.size() > 96:
		_fires.remove_at(0)
	if not _fire_rain and fire_rain_at < 0.0:
		fire_rain_at = _clock + _rng.randf_range(Settings.fire_rain_delay_min_s, Settings.fire_rain_delay_max_s)


# ---------------- per frame ----------------

func _process(delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(delta)
	DebugCapture.note_proc("weather", _pt0)


func _process_timed(delta: float) -> void:
	if main == null or main.player == null:
		return
	_clock += delta
	_scan_t -= delta
	if _scan_t <= 0.0:
		_scan_t = Settings.weather_scan_s
		_scan_fire()
	if fire_rain_at >= 0.0 and _clock >= fire_rain_at:
		_start(0.0, true)
	if is_raining():
		if _fire_rain and _clock - _fire_seen > Settings.weather_fire_clear_s and _clock - fire_rain_at > 0.0:
			_stop()   # no fire seen for a while: the rain has done its work
		elif not _fire_rain and _until > 0.0 and _clock >= _until:
			_stop()
		_drop_t -= delta
		if _drop_t <= 0.0 and intensity > 0.5 and _douse():
			_drop_t = 0.25
		if kind == "storm" and intensity > 0.5:
			_strike_t -= delta
			if _strike_t <= 0.0:
				_strike_t = Settings.lightning_every_s * _rng.randf_range(0.4, 1.6)
				if _rng.randf() < 0.5:
					_far_strike()
				else:
					strike()
	else:
		_roll_t -= delta
		if _roll_t <= 0.0:
			_roll_t = Settings.weather_roll_s * _rng.randf_range(0.7, 1.3)
			_roll()
	intensity = move_toward(intensity, target, delta / Settings.weather_fade_s)
	_show(delta)


## Random weather: a shower with a chance from the column's humidity.
func _roll() -> void:
	var gen = main.world.generator.gen if main.world and main.world.generator else null
	var hum := 0.5
	if gen:
		var p: Vector3 = main.player.position
		hum = clampf(float(gen.column_info(int(p.x), int(p.z)).get("humidity", 0.5)), 0.0, 1.0)
	# ADR 0156: the season moves the odds: a dry summer of rare showers, most of them thunderstorms
	var s: int = main.atmosphere.calendar.season() if main.atmosphere else 1
	if _rng.randf() < lerpf(Settings.weather_rain_chance_dry, Settings.weather_rain_chance_wet, hum) * float(Settings.season_rain_mult[s]):
		_start(Settings.weather_rain_s * _rng.randf_range(0.6, 1.4), false, hum > 0.4 and _rng.randf() < Settings.storm_chance * float(Settings.season_storm_mult[s]))


# ---------------- lightning (ADR 0149) ----------------

## A strike somewhere far off: the sky flickers and thunder rolls in a few seconds later. The warning.
func _far_strike() -> void:
	stats["far_strikes"] += 1
	if main.atmosphere:
		main.atmosphere.flash = 0.35
	get_tree().create_timer(_rng.randf_range(2.0, 6.0)).timeout.connect(Sound.play.bind("weather/thunder_far"))


## The strike's target from a surface map (SimWorld.surface_map: per column material | depth << 8 | y << 16, 0 where
## nothing was found; columns from min_xz in world voxels, `step` apart): the highest surface voxel, a metal one
## (`metal` indexed by material id) counted `metal_up` voxels higher; a rod tip within rod_r voxels (flat distance)
## of that spot takes it instead. {pos, rod}; pos.y < 0 when the map was empty.
static func pick_target(map: PackedInt32Array, min_xz: Vector2i, cols: int, step: int, rod_tips: Array, rod_r: float,
		metal := PackedByteArray(), metal_up := 0) -> Dictionary:
	var best := Vector3i(0, -1, 0)
	var best_score := -1
	for i in map.size():
		var e := map[i]
		if e == 0:
			continue
		var score := (e >> 16) + (metal_up if metal.size() > (e & 255) and metal[e & 255] else 0)
		if score > best_score:
			best_score = score
			best = Vector3i(min_xz.x + (i % cols) * step, e >> 16, min_xz.y + (i / cols) * step)
	if best.y < 0:
		return {"pos": best, "rod": false}
	var rod := false
	var rod_d := rod_r * rod_r
	for t in rod_tips:
		var d := Vector2(t.x - best.x, t.z - best.z).length_squared()
		if d <= rod_d:
			rod_d = d
			best = t
			rod = true
	return {"pos": best, "rod": rod}


## Metal materials by id: ores, meteoric iron, rails, the rod. shortcut: by name, as materials have no metal flag;
## machines and cables are station voxels of many looks and do not count yet. Add a flag when metal roofs exist.
static func metal_mask() -> PackedByteArray:
	var m := PackedByteArray()
	m.resize(256)
	var defs := GameMaterials.defs()
	for i in mini(defs.size(), 255):
		var n: String = defs[i]["name"]
		if n.ends_with("_ore") and not n.begins_with("coal") or "iron" in n or "copper" in n or n.ends_with("rail") or n == "lightning_rod":
			m[i + 1] = 1
	return m


func rod_tips() -> Array:
	var out := []
	if main.get("blocks") != null:
		for e in main.blocks.all_of("lightning_rod"):
			out.append(e.tip())
	return out


## Lightning at `at` (/lightning here), or on the highest exposed voxel sampled around the player. Returns where
## it hit (y < 0: nothing to hit, the sim was busy).
func strike(at := Vector3i(0, -1, 0)) -> Vector3i:
	var bubble: SimBubble = main.bubble
	if bubble == null or not bubble.can_sample():
		return Vector3i(0, -1, 0)
	var t := {"pos": at, "rod": false}
	if at.y < 0:
		var t0 := Time.get_ticks_usec()
		var step := int(Settings.m2v(0.75))   # 64 x 64 columns at 24 m: ~4 ms native (0.5 m cost 12.8 ms, 2026-11-07)
		var r := int(Settings.m2v(Settings.lightning_target_m))
		var o: Vector3i = bubble.origin * Settings.CHUNK
		var size: Vector3i = bubble.sim.get_size_voxels()
		var c := Vector3i(main.player.position.floor()) - o
		var lo := Vector2i(clampi(c.x - r, 0, size.x - 1), clampi(c.z - r, 0, size.z - 1))
		var hi := Vector2i(clampi(c.x + r, 0, size.x - 1), clampi(c.z + r, 0, size.z - 1))
		var cols := Vector2i((hi.x - lo.x) / step + 1, (hi.y - lo.y) / step + 1)
		var map: PackedInt32Array = bubble.sim.surface_map(lo, cols, step, o.y)
		t = pick_target(map, Vector2i(lo.x + o.x, lo.y + o.z), cols.x, step, rod_tips(), Settings.m2v(Settings.lightning_rod_m), _metal, int(Settings.m2v(Settings.lightning_metal_m)))
		stats["scan_ms_max"] = maxf(stats["scan_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)
		if t["pos"].y < 0:
			return t["pos"]
	var p: Vector3i = t["pos"]
	var pf := Vector3(p) + Vector3(0.5, 1.0, 0.5)
	stats["strikes"] += 1
	stats["last_strike"] = [p.x, p.y, p.z, t["rod"]]
	if main.atmosphere:
		main.atmosphere.flash = 1.0
	Sound.play("weather/strike", pf)
	var dist_m := Settings.v2m(main.player.position.distance_to(pf))
	get_tree().create_timer(0.3 + dist_m / 340.0).timeout.connect(Sound.play.bind("weather/thunder", pf))
	if t["rod"]:
		stats["grounded"] += 1
		return p
	# fire: flames in the air over the spot (they catch on what burns beside them); the hit voxel itself alight
	bubble.blast(pf, Settings.m2v(Settings.lightning_fire_m), FIRE, PackedInt32Array([GameMaterials.AIR]))
	var here := bubble.get_voxel(p)
	if here > 0 and GameMaterials.defs()[here - 1].get("burnable", false):
		bubble.set_voxel(p, FIRE)
	fire_started(Vector3(p))
	if main.player.position.distance_to(pf) <= Settings.m2v(Settings.lightning_hurt_m):
		main.player_inv.damage(Settings.lightning_damage, "lightning")
	return p


## Samples flames, smoke and burning cells around the player.
func _scan_fire() -> void:
	var bubble: SimBubble = main.bubble
	if bubble == null or not bubble.can_sample():
		return
	var t0 := Time.get_ticks_usec()
	_seed += 1
	var c := Vector3i(main.player.position.floor())
	var r: PackedInt32Array = bubble.sample_cells(c, int(Settings.m2v(Settings.weather_fire_scan_m)), Settings.weather_fire_samples, _seed, _mask)
	_hits = r.size() / 7
	if _history.size() < 200 and (_history.is_empty() or _clock - float(_history[-1][0]) >= 5.0):
		_history.append([snappedf(_clock, 0.1), _hits, snappedf(intensity, 0.01)])
	for i in range(0, r.size(), 7):
		var p := Vector3i(r[i], r[i + 1], r[i + 2])
		if r[i + 3] == SMOKE and not (r[i + 6] & 1):
			p.y -= int(Settings.m2v(1.0))   # smoke drifts up: the fire is below it
		_note_fire(p)
	stats["scan_ms_max"] = maxf(stats["scan_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)


## Balls of water over the fires seen lately: they fall onto them and put them out. False when the
## sim was busy (tried again next frame).
func _douse() -> bool:
	var bubble: SimBubble = main.bubble
	if bubble == null or _fires.is_empty() or not bubble.can_sample():
		return _fires.is_empty()   # never while a tick runs: blast would wait for it
	var t0 := Time.get_ticks_usec()
	var n := ceili(Settings.rain_drops_per_s * 0.25 * intensity)
	var placed := 0
	for i in n:
		if _fires.is_empty():
			break
		var f: Vector3i = _fires.pop_back()   # the newest first; each fire cell gets one drop
		for up in [1, 2]:
			var p := f + Vector3i(0, up, 0)
			var m := bubble.get_voxel(p)
			if m == GameMaterials.AIR or _air_like.has(m):
				if bubble.set_voxel(p, GameMaterials.WATER):
					placed += 1
				break
	stats["drops"] += placed
	if _clock - _fire_seen > Settings.weather_fire_clear_s * 0.5:
		_fires.clear()   # the fires seen are out: stop pouring there
	stats["drop_ms_max"] = maxf(stats["drop_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)
	return true


func _show(delta: float) -> void:
	var player: Player = main.player
	var cold: bool = main.survival != null and float(main.survival.env_c) < 0.0
	var under: bool = main.survival != null and float(main.survival.underground_m) > 2.0
	var cam: Vector3 = player.camera.global_position
	var shown := intensity * (0.0 if under else 1.0)
	_rain.global_position = cam + Vector3(0, Settings.m2v(10.0), 0)
	_snow.global_position = cam + Vector3(0, Settings.m2v(8.0), 0)
	_rain.emitting = shown > 0.02 and not cold
	_snow.emitting = shown > 0.02 and cold
	_rain.amount_ratio = shown
	_snow.amount_ratio = shown
	if _warm_t > 0.0:
		_warm_t -= delta
		_rain.emitting = true
		_snow.emitting = true
		_rain.amount_ratio = maxf(shown, 0.01)
		_snow.amount_ratio = maxf(shown, 0.01)
	if main.atmosphere:
		main.atmosphere.overcast = intensity
	if _sound.stream:
		var vol := shown * (0.0 if cold else 1.0) * (0.35 if (main.survival and main.survival.in_room) else 1.0)
		if vol > 0.01:
			_sound.volume_db = Settings.rain_db + linear_to_db(vol)
			if not _sound.playing:
				_sound.play(_rng.randf() * 8.0)
		elif _sound.playing:
			_sound.stop()
	# out in the rain the coat gets wet (Noita stains)
	_wet_t -= delta
	if _wet_t <= 0.0:
		_wet_t = 2.0
		if shown > 0.5 and not cold and main.survival and not main.survival.in_room:
			player.stains.apply("wet")


func summary() -> Dictionary:
	var s := stats.duplicate()
	s["kind"] = kind
	s["intensity"] = snappedf(intensity, 0.01)
	s["fire_rain_in_s"] = snappedf(fire_rain_in(), 0.1)
	s["fire_rain"] = _fire_rain
	s["fires_tracked"] = _fires.size()
	s["history"] = _history
	s["fire_seen_s_ago"] = snappedf(_clock - _fire_seen, 0.1) if _fire_seen > -1e8 else -1.0
	return s
