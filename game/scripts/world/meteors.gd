class_name Meteors
extends Node3D

## Meteorite falls (ADR 0157). Some nights (Settings.meteor_chance_per_night, rolled per night from
## the world seed) a streak crosses the sky toward a spot a few hundred metres off, a boom follows at
## the speed of sound, and a glow column stands over the spot for a while (the spot is outside the drawn
## box, ADR 0073, so the sky tells it). A chat line and a "meteorite" waypoint point the way.
## The spot is picked from the seed and the fall's time, and saved. The crater is a runtime edit, made
## once when the whole spot lies loaded in the sim window: a blasted bowl, scorched ground, a ball of
## meteoric iron at the bottom with chunks in the walls, molten (lava, heat damage) for
## Settings.meteor_hot_h game hours, sometimes a fire in the trees. A cold arrival finds basalt instead.

const STREAK_S := 3.0       # seconds the streak takes across the sky
const TAIL := 0.3          # share of the path the tail covers
const NEAR_M := 40.0        # the streak is drawn this far from the camera (inside the far plane)
const SOUND_MPS := 343.0
const CHECK_S := 0.5        # the crater, cooling and heat checks
const COLUMN_M := 140.0
const COLUMN_RADIUS_M := 3.0
const COLUMN_COLOUR := Color(1.0, 0.42, 0.12)
const FIRE_REACH_M := 5.0   # past the bowl: trees this close catch fire

const STREAK_SHADER := """
shader_type spatial;
render_mode unshaded, blend_add, cull_disabled, depth_draw_never, fog_disabled, shadows_disabled;
uniform float glow = 1.0;
varying float v_along;
void vertex() {
	v_along = VERTEX.y + 0.5;   // 0 at the tail, 1 at the head
	VERTEX.xz *= mix(0.2, 1.8, v_along * v_along);   // a fat burning head, a thin fading tail
}
void fragment() {
	float flicker = 0.8 + 0.2 * sin(TIME * 47.0 + v_along * 30.0) * sin(TIME * 23.0);
	vec3 trail = mix(vec3(1.0, 0.35, 0.08), vec3(0.75, 0.9, 1.0), v_along);   // ember tail, blue-white head
	ALBEDO = (trail * pow(v_along, 1.3) * 6.0 + vec3(5.0) * smoothstep(0.88, 1.0, v_along)) * glow * flicker;
}
"""

var main: Node
var events: Array[Dictionary] = []   # {t (calendar days), x, z (voxels), seed, fire, applied, cooled, core [x,y,z]}
var dirty := false
var stats := {"falls": 0, "craters": 0, "cooled": 0, "heat_hits": 0, "crater_voxels": 0}
var _last_now := -1.0
var _check_t := 0.0
var _streaks: Array[Dictionary] = []   # {node, mat, from, to, age, boom_at, boomed, event}
var _columns := {}                     # event seed -> MonolithBeacon
var _puff := 0
var _lights := {}                      # event seed -> OmniLight3D over a hot core in the drawn box
var _streak_mesh: BoxMesh


func setup(p_main: Node) -> void:
	main = p_main
	_streak_mesh = BoxMesh.new()
	var sm := ShaderMaterial.new()
	sm.shader = Shader.new()
	sm.shader.code = STREAK_SHADER
	_streak_mesh.material = sm


## MaterialWarmup draws these under the loading screen (a first draw mid-game compiles a pipeline).
func warm_meshes() -> Array:
	var col := MonolithBeacon.new(Vector3.ZERO, null, COLUMN_M, COLUMN_RADIUS_M)
	var out: Array = [_streak_mesh, col.mesh]
	col.free()
	return out


# ---------------- deterministic picks ----------------

## When tonight's fall comes (calendar days, total), or -1 for a night without one. Night `day` runs
## from its evening (time 0.85) past midnight (1.15).
static func fall_time(world_seed: int, day: int, chance: float) -> float:
	var rng := RandomNumberGenerator.new()
	rng.seed = hash([world_seed, day, "meteor"])
	if rng.randf() >= chance:
		return -1.0
	return float(day) + rng.randf_range(0.85, 1.15)


## Where a fall at time `t` lands, seen from `from` (world voxels): [Vector2 x z, per-fall seed].
static func spot_for(world_seed: int, t: float, from: Vector3, min_m: float, max_m: float) -> Array:
	var rng := RandomNumberGenerator.new()
	var s := hash([world_seed, roundi(t * 86400.0), "meteor_spot"])
	rng.seed = s
	var a := rng.randf() * TAU
	var d := Settings.m2v(rng.randf_range(min_m, max_m))
	return [Vector2(from.x + sin(a) * d, from.z + cos(a) * d).floor(), s]


static func compass(dx: float, dz: float) -> String:
	var names := ["north", "north-east", "east", "south-east", "south", "south-west", "west", "north-west"]
	var a := atan2(dx, -dz)   # 0 = north (-z), clockwise
	return names[posmod(roundi(a / (TAU / 8.0)), 8)]


# ---------------- falls ----------------

func _calendar() -> Calendar:
	var atmo = main.get("atmosphere") if main else null
	return atmo.calendar if atmo else null


func _now() -> float:
	var cal := _calendar()
	return cal.total() if cal else 0.0


func _player_pos() -> Vector3:
	var p = main.get("player") if main else null
	return p.position if p else Vector3.ZERO


## The night roll: a fall whose time was crossed since the last frame (not across a sleep or a jump).
func _roll(now: float) -> void:
	if _last_now < 0.0 or now < _last_now or now - _last_now > 0.05:
		_last_now = now
		return
	for day in range(floori(_last_now - 1.2), floori(now) + 1):
		var t := fall_time(Settings.world_seed, day, Settings.meteor_chance_per_night)
		if t > _last_now and t <= now:
			var cal := _calendar()
			if cal == null or cal.sun_dir_at(t).y < -0.05:   # only in the dark
				fall_from(_player_pos(), t)
	_last_now = now


## A fall at time `t` near `from`: the event (saved), the streak, the boom, the column and the hint.
func fall_from(from: Vector3, t: float, at := Vector2.INF) -> Dictionary:
	var pick := spot_for(Settings.world_seed, t, from, Settings.meteor_min_m, Settings.meteor_max_m)
	var spot: Vector2 = pick[0] if at == Vector2.INF else at.floor()
	var rng := RandomNumberGenerator.new()
	rng.seed = int(pick[1])
	var e := {"t": t, "x": int(spot.x), "z": int(spot.y), "seed": int(pick[1]), "fire": rng.randf() < Settings.meteor_fire_chance,
		"applied": false, "cooled": false, "landing": true}
	events.append(e)
	dirty = true
	stats["falls"] += 1
	_start_streak(e, from)
	var dx := spot.x - from.x
	var dz := spot.y - from.z
	var dist_m := Vector2(dx, dz).length() * Settings.VOXEL_M
	var wps = main.get("waypoints") if main else null
	if wps:
		var world = main.get("world")
		var gy: int = world.surface_height(e["x"], e["z"]) if world else int(from.y)
		wps.set_unique("meteorite", "Meteorite", Vector3i(e["x"], gy, e["z"]))
	var chat = main.get("chat") if main else null
	if chat and dist_m > 1.0:
		chat.add_message("A meteorite fell about %d m to the %s; its glow marks the spot (waypoint on the map)" % [roundi(dist_m / 10.0) * 10, compass(dx, dz)])
	return e


func _start_streak(e: Dictionary, from: Vector3) -> void:
	var dx: float = e["x"] - from.x
	var dz: float = e["z"] - from.z
	var dist_m := Vector2(dx, dz).length() * Settings.VOXEL_M
	var az := atan2(dx, dz)
	var rng := RandomNumberGenerator.new()
	rng.seed = int(e["seed"]) + 1
	var start_az := az + rng.randf_range(-0.7, 0.7)
	var mi := MeshInstance3D.new()
	mi.mesh = _streak_mesh
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	mi.extra_cull_margin = Settings.m2v(NEAR_M) * 2.0
	mi.visible = false
	add_child(mi)
	_streaks.append({"node": mi, "from": _dir(start_az, deg_to_rad(rng.randf_range(45.0, 65.0))), "to": _dir(az, deg_to_rad(1.5)),
		"age": 0.0, "boom_at": STREAK_S + dist_m / SOUND_MPS, "boomed": false, "event": e})
	Sound.play("meteor/streak")


static func _dir(az: float, el: float) -> Vector3:
	return Vector3(sin(az) * cos(el), sin(el), cos(az) * cos(el))


# ---------------- every frame ----------------

func _process(delta: float) -> void:
	if main == null:
		return
	var now := _now()
	_roll(now)
	_tick_streaks(delta)
	_tick_columns(now)
	for k in _lights:   # the core's glow breathes and flickers like embers
		var t := Time.get_ticks_msec() / 1000.0 + float(k % 100)
		var l: OmniLight3D = _lights[k]
		var heat := clampf(1.0 - (now - float(l.get_meta("t", now))) * 24.0 / Settings.meteor_hot_h, 0.25, 1.0)   # dims as it cools
		l.light_energy = (7.0 + 1.5 * sin(t * 2.1) + 0.8 * sin(t * 13.0)) * heat
	_check_t -= delta
	if _check_t <= 0.0:
		_check_t = CHECK_S
		check(now)


func _camera() -> Camera3D:
	var p = main.get("player")
	return p.camera if p else get_viewport().get_camera_3d()


func _tick_streaks(delta: float) -> void:
	var cam := _camera()
	for s in _streaks.duplicate():
		s["age"] = float(s["age"]) + delta
		var p := float(s["age"]) / STREAK_S
		var mi: MeshInstance3D = s["node"]
		if p < 1.0 and cam:
			var head: Vector3 = (s["from"] as Vector3).slerp(s["to"], p)
			var tail: Vector3 = (s["from"] as Vector3).slerp(s["to"], maxf(0.0, p - TAIL))
			var near := minf(Settings.m2v(NEAR_M), cam.far * 0.7)   # inside the far plane at short view distances
			var c := cam.global_position
			var ph := c + head * near
			var pt := c + tail * near
			var axis := ph - pt
			var w := near * 0.012 * (1.0 + 1.5 * p)   # it grows as it nears
			var side := axis.cross(ph - c).normalized()
			var up := side.cross(axis.normalized())
			mi.global_transform = Transform3D(Basis(side * w, axis, up * w), (ph + pt) * 0.5)
			mi.visible = true
		else:
			mi.visible = false
			var e: Dictionary = s["event"]
			if e.has("landing"):
				e.erase("landing")
				e["landed_ms"] = Time.get_ticks_msec()   # the column flashes as it lands
		if not s["boomed"] and float(s["age"]) >= float(s["boom_at"]):
			s["boomed"] = true
			Sound.play("meteor/boom")
		if s["boomed"] and p >= 1.0:
			mi.queue_free()
			_streaks.erase(s)


## The glow column over each fresh fall, fading out over Settings.meteor_glow_h.
func _tick_columns(now: float) -> void:
	var cam := _camera()
	for e in events:
		var age_h := (now - float(e["t"])) * 24.0
		var on := cam != null and not e.has("landing") and age_h >= 0.0 and age_h < Settings.meteor_glow_h
		if on and e["applied"]:   # once the crater is in the drawn box it shows itself (and smokes, _heat)
			on = Vector2(cam.global_position.x - e["x"], cam.global_position.z - e["z"]).length() > Settings.m2v(Settings.view_distance_m)
		var key: int = e["seed"]
		var col: MonolithBeacon = _columns.get(key)
		if on and col == null:
			var world = main.get("world")
			var gy: float = world.surface_height(e["x"], e["z"]) if world else 0.0
			col = MonolithBeacon.new(Vector3(e["x"] + 0.5, gy, e["z"] + 0.5), cam, COLUMN_M, COLUMN_RADIUS_M)
			add_child(col)
			_columns[key] = col
		elif not on and col != null:
			col.queue_free()
			_columns.erase(key)
		if col:
			var f := 1.0 - age_h / Settings.meteor_glow_h
			var flash := 1.0 + 6.0 * exp(-(Time.get_ticks_msec() - int(e.get("landed_ms", -100000))) / 700.0)
			col.material.set_shader_parameter("colour", COLUMN_COLOUR * (0.35 + 0.65 * f) * f * flash)


# ---------------- the crater ----------------

## Craters whose spot is loaded now, cores that cooled, a player on a hot core.
func check(now: float) -> void:
	for e in events:
		if e.has("landing"):
			continue
		if not e["applied"]:
			apply(e, now)
		elif not e["cooled"] and (now - float(e["t"])) * 24.0 >= Settings.meteor_hot_h:
			_cool(e)
		elif not e["cooled"]:
			_heat(e)


## The crater at a fall's spot, once its whole reach is loaded in the sim window. True when made now.
func apply(e: Dictionary, now: float) -> bool:
	var bubble = main.get("bubble")
	if bubble == null:
		return false
	var r := Settings.m2v(Settings.meteor_crater_m)
	var reach := r + Settings.m2v(FIRE_REACH_M) + 1.0
	var gy: Variant = _ground_y(e, bubble)
	if gy == null:
		stats["last_wait"] = "ground column not loaded"
		return false
	var c := Vector3(int(e["x"]) + 0.5, float(gy) + r * 0.4, int(e["z"]) + 0.5)
	# the bowl's depth under the centre and the fire's reach on the four sides (a far diagonal chunk can stay
	# unloaded at short view distances; the native blast skips chunks still loading anyway)
	for o in [Vector3(0, -r - 2.0, 0), Vector3(0, r + 2.0, 0), Vector3(reach, 0, 0), Vector3(-reach, 0, 0), Vector3(0, 0, reach), Vector3(0, 0, -reach)]:
		var q := Vector3i((c + o).floor())
		if bubble.get_voxel(q) < 0:
			stats["last_wait"] = "not loaded at %s" % q
			return false   # outside the window or still loading
	var hot := (now - float(e["t"])) * 24.0 < Settings.meteor_hot_h
	var rng := RandomNumberGenerator.new()
	rng.seed = int(e["seed"]) + 2
	var core_r := Settings.m2v(Settings.meteor_core_m)
	var cc := Vector3(c.x, c.y - r + core_r * 0.7, c.z)   # the ball stands out of the lava pool
	var n := 0
	Net.recording = true   # ADR 0145: the crater reaches the other players
	n += bubble.blast(c, r)
	n += bubble.blast(c, r + Settings.m2v(1.0), GameMaterials.SCORCHED_EARTH, _ids(func(d: Dictionary, id: int) -> bool:
		return GameMaterials.is_soil(id) or id == GameMaterials.SNOW))
	n += bubble.blast(c, r + Settings.m2v(2.0), GameMaterials.AIR, _ids(func(d: Dictionary, _id: int) -> bool: return d.get("replaceable", false)))
	var iron := GameMaterials.METEORIC_IRON_HOT if hot else GameMaterials.METEORIC_IRON   # it glows until it cools
	n += bubble.blast(cc, core_r, iron)
	for i in Settings.meteor_chunks:
		var a := rng.randf() * TAU
		var down := rng.randf_range(0.35, 0.9)   # on the lower bowl wall
		var dir := Vector3(sin(a) * sqrt(1.0 - down * down), -down, cos(a) * sqrt(1.0 - down * down))
		n += bubble.blast(c + dir * r, Settings.m2v(rng.randf_range(0.2, 0.35)), iron)
	if hot:
		n += bubble.blast(cc - Vector3(0, core_r * 0.5, 0), core_r * 1.3, GameMaterials.LAVA, PackedInt32Array([GameMaterials.AIR]))
		if e["fire"]:
			n += bubble.blast(c, r + Settings.m2v(FIRE_REACH_M), GameMaterials.FIRE, _ids(func(d: Dictionary, _id: int) -> bool:
				return d.get("burnable", false) and d.get("cell_type", "solid") == "solid" and not String(d["name"]).ends_with("_ore")))
	Net.recording = false
	var pieces = main.get("pieces")
	if pieces:
		pieces.check(PlaytestTools.rim_seeds(c, r), c)
	var fx = main.get("fx")
	if fx:
		fx.burst("tnt_flash", c)
		fx.burst("tnt_smoke", c)
	Sound.play("tnt/explode", c)
	e["applied"] = true
	e["cooled"] = not hot
	e["core"] = [cc.x, cc.y, cc.z]
	dirty = true
	stats["craters"] += 1
	stats["crater_voxels"] += n
	return true


## The ground under the spot: the first solid, non-tree voxel down from a little above the
## generator's surface; null while that column is not loaded.
func _ground_y(e: Dictionary, bubble) -> Variant:
	var world = main.get("world")
	var x: int = e["x"]
	var z: int = e["z"]
	var top: int = (world.surface_height(x, z) if world else 0) + Settings.m2v(8.0)
	for y in range(top, top - Settings.m2v(24.0), -1):
		var m: int = bubble.get_voxel(Vector3i(x, y, z))
		if m < 0:
			return null
		if m == 0:
			continue
		var d: Dictionary = GameMaterials.defs()[m - 1]
		if d.get("cell_type", "solid") in ["liquid", "gas"] or d.has("rigid") or d.get("replaceable", false) or not GameMaterials.blocks_bodies(m):
			continue
		return y
	return null


## Material ids whose definition passes `keep(def, id)`.
static func _ids(keep: Callable) -> PackedInt32Array:
	var out := PackedInt32Array()
	var defs := GameMaterials.defs()
	for i in mini(defs.size(), 255):
		if keep.call(defs[i], i + 1):
			out.append(i + 1)
	return out


func _core(e: Dictionary) -> Vector3:
	var a: Array = e.get("core", [0, 0, 0])
	return Vector3(a[0], a[1], a[2])


## The molten core turns to basalt once its hours ran out (when its spot is loaded again).
func _cool(e: Dictionary) -> void:
	var bubble = main.get("bubble")
	var cc := _core(e)
	if bubble == null or bubble.get_voxel(Vector3i(cc.floor())) < 0:
		return
	Net.recording = true
	bubble.blast(cc, Settings.m2v(Settings.meteor_crater_m), GameMaterials.BASALT, PackedInt32Array([GameMaterials.LAVA, GameMaterials.LAVA_STATIC]))
	# the core and the wall chunks (within two bowl radii of the core) stop glowing
	bubble.blast(cc, Settings.m2v(Settings.meteor_crater_m) * 2.0, GameMaterials.METEORIC_IRON, PackedInt32Array([GameMaterials.METEORIC_IRON_HOT]))
	Net.recording = false
	e["cooled"] = true
	dirty = true
	stats["cooled"] += 1
	_drop_light(e)


func _drop_light(e: Dictionary) -> void:
	var l: OmniLight3D = _lights.get(int(e["seed"]))
	if l:
		l.queue_free()
		_lights.erase(int(e["seed"]))


## A hot core smokes; standing on (or in) it burns.
func _heat(e: Dictionary) -> void:
	var p = main.get("player")
	var inv = main.get("player_inv")
	var fx = main.get("fx")
	var near: bool = p != null and p.position.distance_to(_core(e)) < Settings.m2v(Settings.view_distance_m)
	if fx and near:
		_puff += 1
		if _puff % 6 == 0:   # a thin plume: a burst every half second buried the glowing core in a cloud
			fx.burst("tnt_smoke", _core(e) + Vector3(0, Settings.m2v(1.5), 0))
		fx.burst("tnt_sparks", _core(e) + Vector3(0, Settings.m2v(0.4), 0))
	if near and not _lights.has(int(e["seed"])):
		var l := OmniLight3D.new()   # the lava alone lights little at night: the core glows on its own
		l.light_color = Color(1.0, 0.45, 0.15)
		l.omni_range = Settings.m2v(20.0)
		l.omni_attenuation = Settings.torch_light_attenuation
		l.shadow_enabled = false
		add_child(l)
		l.global_position = _core(e) + Vector3(0, Settings.m2v(1.5), 0)
		l.set_meta("t", float(e["t"]))
		_lights[int(e["seed"])] = l
	elif not near:
		_drop_light(e)
	if p == null or inv == null or not GameMode.rule(p.game_mode, "takes_damage"):
		return
	var reach := Settings.m2v(Settings.meteor_core_m + 0.6)
	if p.position.distance_to(_core(e)) > reach:
		return
	inv.damage(Settings.meteor_heat_damage, "a white-hot meteorite")
	stats["heat_hits"] += 1
	if inv.is_dead():
		p.died.emit()


# ---------------- playtest, save ----------------

## /meteor [here|near]: here lands 6 m ahead of you, near 120 m ahead (outside the box), no argument
## the usual few hundred metres in any direction.
func command(args: PackedStringArray) -> Dictionary:
	var p = main.get("player")
	if p == null:
		return CommandRegistry.fail("No player")
	var from: Vector3 = p.position
	var mode := args[0] if args.size() > 0 else ""
	var f: Vector3 = -p.camera.global_transform.basis.z
	var ahead := Vector2(f.x, f.z).normalized() if Vector2(f.x, f.z).length() > 0.01 else Vector2(0, -1)
	var at := Vector2.INF
	match mode:
		"here":
			at = Vector2(from.x, from.z) + ahead * Settings.m2v(6.0)
		"near":
			at = Vector2(from.x, from.z) + ahead * Settings.m2v(120.0)
		"stats":
			return CommandRegistry.ok(JSON.stringify(snapshot()))
		"":
			pass
		_:
			return CommandRegistry.fail("/meteor [here|near|stats]")
	var e := fall_from(from, _now(), at)
	return CommandRegistry.ok("Meteorite on its way to %d %d" % [e["x"], e["z"]])


func snapshot() -> Dictionary:
	var out := stats.duplicate()
	out["events"] = events.size()
	out["core_lights"] = _lights.size()
	out["pending"] = events.filter(func(e: Dictionary) -> bool: return not e["applied"]).size()
	out["next_tonight"] = fall_time(Settings.world_seed, floori(_now() - 0.5), Settings.meteor_chance_per_night)
	return out


func to_dict() -> Dictionary:
	var out := []
	for e in events:
		var c := e.duplicate()
		c.erase("landing")   # a fall saved mid-streak lands on load
		out.append(c)
	return {"version": 1, "events": out}


func load_dict(d: Dictionary) -> void:
	events.clear()
	if int(d.get("version", 0)) != 1:
		return
	for v in d.get("events", []):
		if v is Dictionary and v.has("x") and v.has("z") and v.has("t"):
			var e: Dictionary = v.duplicate()
			e["x"] = int(e["x"])
			e["z"] = int(e["z"])
			e["seed"] = int(e.get("seed", 0))
			e["applied"] = bool(e.get("applied", false))
			e["cooled"] = bool(e.get("cooled", false))
			e["fire"] = bool(e.get("fire", false))
			events.append(e)
