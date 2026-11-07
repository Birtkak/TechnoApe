class_name Volcano
extends Node

## Volcano eruptions (ADR 0149). Volcanoes are generated cones with a still-lava crater (ADR 0146); this is the
## runtime part. Every volcano_roll_s, if a crater lies within volcano_near_m of the player, it may erupt:
## first volcano_warn_s of rumble (a lava loop at the crater) and ash (smoke balls over the lava: the warning),
## then volcano_flow_s of moving lava poured just outside the rim on the player's side (SimBubble.blast balls of
## LAVA, so it runs down the flank, lights wood, meets water as the lava sim does) and lava bombs flung onto the
## flank, until volcano_lava_cells cells are out: bounded, inside the sim window. Nothing lands in the crater: its
## still lake (LAVA_STATIC) woken as a whole cost 65 ms a tick (2026-11-07). The generator is untouched.
## /erupt starts one at the nearest crater. GDScript only orchestrates (principle 8): one blast a half second.

const LAVA := GameMaterials.LAVA
const SMOKE := 11

var main: Node
var phase := ""            # "", "warn", "flow"
var crater := Vector3i.ZERO   # the crater's centre, y at the lava
var rim := Vector3i.ZERO      # where the lava comes over, y on the rim
var _spout := Vector3i.ZERO   # the pour point: 3 m outside the rim on the player's side, on the ground there
var _rim_d := 0.0             # the rim's distance from the centre in voxels (bombs land beyond it)
var poured := 0            # lava cells poured in all eruptions
var _poured0 := 0          # poured when this eruption began
var bombs := 0
var eruptions := 0
var _t := 0.0
var _roll_t := 0.0
var _pour_t := 0.0
var _bomb_t := 0.0
var _rng := RandomNumberGenerator.new()
var _rumble: AudioStreamPlayer3D
var ms_max := {"ash": 0.0, "pour": 0.0, "bomb": 0.0}   # the slowest sphere edit of each kind


func setup(p_main: Node) -> void:
	main = p_main
	_rng.seed = 4242
	_roll_t = Settings.volcano_roll_s * 0.5
	_rumble = Sound.loop_voice("volcano/rumble")
	if _rumble:
		_rumble.pitch_scale = 0.45
		_rumble.stream_paused = true
		add_child(_rumble)


## The nearest crater: {"pos": centre with y at the lava surface, "dist_m"}, or {} when none within `within_m`.
func nearest(within_m: float) -> Dictionary:
	var gen = main.world.generator.gen if main.world and main.world.generator else null
	if gen == null:
		return {}
	var p: Vector3 = main.player.position
	var t: Vector3i = gen.find_nearest("volcano", int(p.x), int(p.z), int(Settings.m2v(within_m)))
	if t.y < 0:
		return {}
	var info: Dictionary = gen.column_info(t.x, t.z)
	t.y = maxi(t.y, int(info.get("lava", 0)))
	return {"pos": t, "dist_m": Settings.v2m(Vector2(t.x - p.x, t.z - p.z).length())}


## Starts an eruption at the nearest crater within volcano_near_m. Returns a message.
func erupt() -> String:
	var n := nearest(4000.0)
	if n.is_empty():
		return "No volcano within 4 km"
	if float(n["dist_m"]) > Settings.volcano_near_m:
		return "The nearest volcano is %d m away: /goto volcano first" % int(n["dist_m"])
	erupt_at(n["pos"])
	return "The volcano at %d, %d rumbles" % [crater.x, crater.z]


func erupt_at(centre: Vector3i) -> void:
	crater = centre
	rim = _find_rim(centre)
	var out := Vector2(rim.x - centre.x, rim.z - centre.z)
	_rim_d = out.length()
	out = (out.normalized() if _rim_d > 1.0 else Vector2(1, 0)) * Settings.m2v(3.0)
	_spout = Vector3i(rim.x + int(out.x), _ground_y(rim.x + int(out.x), rim.z + int(out.y), rim.y), rim.z + int(out.y))
	phase = "warn"
	_t = Settings.volcano_warn_s
	_pour_t = 0.0
	_bomb_t = 2.0
	_poured0 = poured
	eruptions += 1
	if _rumble:
		_rumble.position = Vector3(crater) + Vector3(0.5, 0.5, 0.5)
		_rumble.stream_paused = false
		if not _rumble.playing:
			_rumble.play()


func _ground_y(x: int, z: int, fallback: int) -> int:
	var gen = main.world.generator.gen if main.world and main.world.generator else null
	return int(gen.column_info(x, z).get("height", fallback)) if gen else fallback


## The rim on the player's side: the highest column walking out from the centre toward the player (1 m steps,
## up to 60 m). The centre itself when the generator is not there (tests). /goto volcano lands there too.
func _find_rim(centre: Vector3i) -> Vector3i:
	return rim_toward(main.world.generator.gen if main.world and main.world.generator else null, centre, main.player.position)


static func rim_toward(gen, centre: Vector3i, p: Vector3) -> Vector3i:
	var dir := Vector2(p.x - centre.x, p.z - centre.z)
	dir = dir.normalized() if dir.length() > 1.0 else Vector2(1, 0)
	var best := centre
	if gen == null:
		return best
	for i in range(1, 61):
		var q := Vector2(centre.x, centre.z) + dir * Settings.m2v(float(i))
		var h := int(gen.column_info(int(q.x), int(q.y)).get("height", -1))
		if h > best.y:
			best = Vector3i(int(q.x), h, int(q.y))
		elif h < best.y - int(Settings.m2v(3.0)) and best != centre:
			break   # over the rim and going down
	return best


func _process(delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(delta)
	DebugCapture.note_proc("volcano", _pt0)


func _process_timed(delta: float) -> void:
	if main == null or main.player == null or main.bubble == null:
		return
	if phase == "":
		_roll_t -= delta
		if _roll_t <= 0.0:
			_roll_t = Settings.volcano_roll_s
			if _rng.randf() < Settings.volcano_chance:
				var n := nearest(Settings.volcano_near_m)
				if not n.is_empty():
					erupt_at(n["pos"])
		return
	_t -= delta
	_pour_t -= delta
	_bomb_t -= delta
	var bubble: SimBubble = main.bubble
	if phase == "warn":
		if _pour_t <= 0.0 and bubble.can_sample():
			_pour_t = 0.5
			# 6 m over the lake: smoke touching the still lava wakes the whole lake (56 ms ticks, 2026-11-07)
			_box("ash", crater + Vector3i(0, int(Settings.m2v(6.0)), 0), int(Settings.m2v(1.0)), SMOKE, 0.6)
		if _bomb_t <= 0.0:
			_bomb_t = _rng.randf_range(3.0, 6.0)
			Sound.play("volcano/burst", Vector3(crater))
		if _t <= 0.0:
			phase = "flow"
			_t = Settings.volcano_flow_s
		return
	# flow: a ball of lava at the spout every half second, a bomb onto the flank now and then
	if _pour_t <= 0.0 and bubble.can_sample():
		_pour_t = 0.5
		poured += _box("pour", _spout + Vector3i(0, 1, 0), int(Settings.m2v(0.75)), LAVA, 1.0)
	if _bomb_t <= 0.0 and bubble.can_sample():
		_bomb_t = _rng.randf_range(3.0, 7.0)
		var a := _rng.randf() * TAU
		var d := _rim_d + Settings.m2v(_rng.randf_range(4.0, 16.0))
		var at := Vector3(crater) + Vector3(cos(a) * d, Settings.m2v(14.0), sin(a) * d)
		poured += _blast("bomb", at, Settings.m2v(0.5), LAVA)
		bombs += 1
		Sound.play("volcano/burst", Vector3(crater))
	if _t <= 0.0 or poured - _poured0 >= Settings.volcano_lava_cells:
		phase = ""
		if _rumble:
			_rumble.stream_paused = true


func _blast(what: String, at: Vector3, r: float, mat: int) -> int:
	var t0 := Time.get_ticks_usec()
	var n: int = main.bubble.blast(at, r, mat, PackedInt32Array([GameMaterials.AIR]))
	ms_max[what] = maxf(ms_max[what], (Time.get_ticks_usec() - t0) / 1000.0)
	return n


## A `side`-voxel cube of `mat` resting on `foot` (SimWorld.scatter: single-threaded, in place). Not a sphere edit:
## blast runs on the worker pool, which the burning flank keeps busy: 68-135 ms waits (2026-11-07). Over air only:
## the ash 6 m over the lake, the spout's cube on the ground beside the rim. shortcut: no net op (ADR 0145), so a
## client sees the lava only as the host's sim deltas; send one when eruptions matter in multiplayer.
func _box(what: String, foot: Vector3i, side: int, mat: int, density: float) -> int:
	var t0 := Time.get_ticks_usec()
	var lo: Vector3i = foot - Vector3i(side / 2, 0, side / 2) - main.bubble.origin * Settings.CHUNK
	main.bubble.sim.scatter(lo, lo + Vector3i(side, side, side), mat, density, _rng.randi())
	ms_max[what] = maxf(ms_max[what], (Time.get_ticks_usec() - t0) / 1000.0)
	return int(side * side * side * density)


func summary() -> Dictionary:
	return {"phase": phase, "eruptions": eruptions, "poured": poured, "bombs": bombs, "crater": [crater.x, crater.y, crater.z], "rim": [rim.x, rim.y, rim.z], "spout": [_spout.x, _spout.y, _spout.z], "ms_max": ms_max}
