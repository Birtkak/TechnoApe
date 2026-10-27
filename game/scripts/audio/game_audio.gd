class_name GameAudio
extends Node

## Sound (ADR 0044), the `Sound` autoload: it outlives the title screen and the game. Pure
## presentation: nothing here touches the world or the save. Sounds are data (data/sounds.json,
## assets from tools/make_sounds.py); scripts ask for them by name with play(), but most of the game
## is only listened to: the player's signals, ParticleFx.burst_played, FallingPieces.piece_event and
## every button the UI adds. Steps, swimming, mining hits, eating, pickups and animal calls come from
## watching the player and the mobs; fire, flowing water, falling sand, lava and steam are loops fed by
## SimWorld.sample_cells around the camera (native sampling, only the hits come back here).
##
## Buses: Master > Music, World (effects and loops; a low-pass under water, reverb in enclosed
## spaces), UI. World units are voxels, so ranges convert from metres. Voices come from fixed pools;
## when all are busy the oldest is stolen.

const VOICES_3D := 40
const VOICES_2D := 16
const LOOP_HZ := 10.0
const LOOP_RADIUS_M := 16.0
const LOOP_SAMPLES := 600
const MOB_HZ := 4.0
const PROBE_RANGE_M := 12.0
const PROBE_DIRS := [Vector3(0, 1, 0), Vector3(1, 0, 0), Vector3(-1, 0, 0), Vector3(0, 0, 1), Vector3(0, 0, -1),
	Vector3(0.7, 0.7, 0), Vector3(-0.7, 0.7, 0), Vector3(0, 0.7, 0.7), Vector3(0, 0.7, -0.7), Vector3(0, -1, 0)]
const WHERE := ["any", "air_above", "air_below", "moving", "fresh"]   # moving / fresh: SimWorld::SampleBits (ADR 0081)
## Groups that wall a space in for the reverb: rock, earth and built walls. Not trunks, leaves or
## plants: a forest is open air.
const ENCLOSING := ["stone", "dirt", "grass", "gravel", "sand", "snow", "mud", "glass", "metal", "plank"]
const ROOT := "res://assets/sounds/"

var enabled := true            # sound=0 turns every sound off
var music_enabled := true      # music=0
var errors: Array[String] = []
var main: Node                 # the running game, null on the title screen

var _index := {}               # asset name -> files (assets/sounds/index.json)
var _defs := {}                # sound name -> definition (sounds.json, metres)
var _sounds := {}              # sound name -> compiled (see _compile)
var _events := {}              # step / hit / break / place / land -> parameters
var _groups := {}              # group -> {event -> asset}
var _group_of := PackedStringArray()   # material id -> group
var _particle_sounds := {}     # burst name -> sound
var _mobs := {}
var _loop_defs := {}
var _music := {}
var _streams := {}             # file -> AudioStream (loaded on first use)
var _rng := RandomNumberGenerator.new()

var _voices3: Array[AudioStreamPlayer3D] = []
var _voices2: Array[AudioStreamPlayer] = []
var _started := {}             # voice -> ticks msec it started (stealing: oldest first)
var _last_play := {}           # sound -> ticks msec (cooldowns)
var _last_variant := {}        # sound -> index (no immediate repeats)
var _bus := {}                 # "Music" / "World" / "UI" -> index

var stats_data := {"plays": {}, "played": 0, "culled": 0, "stolen": 0, "cooldown": 0, "missing": {}, "last": [],
	"loops": {}, "music": "", "underwater": false, "reverb": 0.0, "ms_max": 0.0, "ms_avg": 0.0}
var _ms_sum := 0.0
var _ms_frames := 0

# the game: players, loops, environment, music
var _player: Player
var _step_accum := 0.0
var _swim_accum := 0.0
var _hit_t := 0.0
var _bubble_t := 0.0
var _was_in_liquid := false
var _last_vel := Vector3.ZERO
var _picked := 0
var _picked_t := 0.0
var _chest_open := false
var _mob_t := 0.0
var _mob_next := {}            # mob uid -> seconds until its next call
var _loop_t := 0.0
var _loop_seed := 1
var _loop_mask := PackedByteArray()
var _loop_mats := {}           # material id -> Array of loop names
var _loops := {}               # loop name -> {player, level, target, pos, quiet_s}
var _probe_i := 0
var _probe_hits := PackedFloat32Array()
var _reverb := 0.0
var _underwater_voice: AudioStreamPlayer
var _music_player: AudioStreamPlayer
var _music_wait := -1.0        # seconds until the next track; < 0 not scheduled
var _music_mode := ""          # "title" / "game" / ""
var _music_tween: Tween


func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS   # the pause menu still clicks
	_rng.randomize()
	for a in OS.get_cmdline_user_args():
		if a == "sound=0":
			enabled = false
		elif a == "music=0":
			music_enabled = false
	_setup_buses()
	load_file("res://data/sounds.json")
	for e in errors:
		push_error("sounds: " + e)
	if enabled:
		_preload(["ui/", "book/"])
	for i in VOICES_3D:
		var p := AudioStreamPlayer3D.new()
		p.name = "Voice3D_%d" % i
		p.bus = "World"
		p.doppler_tracking = AudioStreamPlayer3D.DOPPLER_TRACKING_DISABLED
		p.attenuation_model = AudioStreamPlayer3D.ATTENUATION_INVERSE_DISTANCE
		add_child(p)
		_voices3.append(p)
	for i in VOICES_2D:
		var p := AudioStreamPlayer.new()
		p.name = "Voice2D_%d" % i
		add_child(p)
		_voices2.append(p)
	_music_player = AudioStreamPlayer.new()
	_music_player.name = "Music"
	_music_player.bus = "Music"
	add_child(_music_player)
	_music_player.finished.connect(_on_music_finished)
	_probe_hits.resize(PROBE_DIRS.size())
	_probe_hits.fill(PROBE_RANGE_M)
	apply_volumes()
	Settings.user_settings_changed.connect(apply_volumes)
	get_tree().node_added.connect(_on_node_added)


func _setup_buses() -> void:
	for n in ["Music", "World", "UI"]:
		var i := AudioServer.get_bus_index(n)
		if i < 0:
			AudioServer.add_bus()
			i = AudioServer.bus_count - 1
			AudioServer.set_bus_name(i, n)
			AudioServer.set_bus_send(i, "Master")
		_bus[n] = i
	var w: int = _bus["World"]
	if AudioServer.get_bus_effect_count(w) == 0:
		var lp := AudioEffectLowPassFilter.new()
		lp.cutoff_hz = 20000.0
		AudioServer.add_bus_effect(w, lp)
		var rv := AudioEffectReverb.new()
		rv.room_size = 0.55
		rv.damping = 0.6
		rv.spread = 0.8
		rv.dry = 1.0
		rv.wet = 0.0
		rv.predelay_msec = 40.0
		AudioServer.add_bus_effect(w, rv)
		AudioServer.set_bus_effect_enabled(w, 0, false)
		AudioServer.set_bus_effect_enabled(w, 1, false)


## Master, music, effects and interface volume from Settings (0..1, linear).
func apply_volumes() -> void:
	AudioServer.set_bus_volume_db(0, linear_to_db(maxf(Settings.volume_master, 0.0001)))
	AudioServer.set_bus_mute(0, Settings.volume_master <= 0.0 or not enabled)
	for pair in [["Music", Settings.volume_music], ["World", Settings.volume_effects], ["UI", Settings.volume_ui]]:
		var i: int = _bus[pair[0]]
		AudioServer.set_bus_volume_db(i, linear_to_db(maxf(pair[1], 0.0001)))
		AudioServer.set_bus_mute(i, pair[1] <= 0.0)


# ---------------- data ----------------

func load_file(path: String) -> bool:
	var fa := FileAccess.open(ROOT + "index.json", FileAccess.READ)
	if fa == null:
		errors.append("no %sindex.json: run tools/make_sounds.py" % ROOT)
	else:
		_index = JSON.parse_string(fa.get_as_text())
	fa = FileAccess.open(path, FileAccess.READ)
	if fa == null:
		errors.append("cannot open %s" % path)
		return false
	var data = JSON.parse_string(fa.get_as_text())
	if not (data is Dictionary):
		errors.append("%s: not a JSON object" % path)
		return false
	return load_dict(data)


func load_dict(data: Dictionary) -> bool:
	_defs = data.get("sounds", {})
	for n in _defs:
		_compile(n, _defs[n])
	_events = data.get("events", {})
	_groups = data.get("groups", {})
	for g in _groups:
		for ev in _groups[g]:
			if not _index.has(_groups[g][ev]):
				errors.append("group %s %s: no asset %s" % [g, ev, _groups[g][ev]])
	var mats: Dictionary = data.get("materials", {})
	var by_type: Dictionary = data.get("cell_types", {})
	var defs := GameMaterials.defs()
	_group_of.resize(defs.size() + 1)
	_group_of[0] = "none"
	for i in defs.size():
		var d: Dictionary = defs[i]
		var g: String = mats.get(d["name"], by_type.get(d.get("cell_type", ""), ""))
		if g == "" or not _groups.has(g):
			errors.append("material %s: no sound group" % d["name"])
			g = "none"
		_group_of[i + 1] = g
	_particle_sounds = data.get("particles", {})
	_mobs = data.get("mobs", {})
	_music = data.get("music", {})
	for n in _particle_sounds.values():
		_check_sound("particles", n)
	for sp in _mobs:
		for k in ["idle", "hurt", "baby", "angry"]:
			if _mobs[sp].has(k):
				var names = _mobs[sp][k]   # a name, or (idle) a list picked from at random
				for one in (names if names is Array else [names]):
					_check_sound("mob " + sp, one)
	for pool in ["title", "day", "night"]:
		for n in _music.get(pool, []):
			if not _index.has(n):
				errors.append("music %s: no asset %s" % [pool, n])
	_loop_defs = data.get("loops", {})
	_loop_mask.resize(256)
	_loop_mask.fill(0)
	var ids := {}
	for i in defs.size():
		ids[defs[i]["name"]] = i + 1
	for ln in _loop_defs:
		var l: Dictionary = _loop_defs[ln]
		_check_sound("loop " + ln, l.get("sound", ""))
		if not (l.get("where", "any") in WHERE):
			errors.append("loop %s: unknown where %s" % [ln, l.get("where", "")])
		for mn in l.get("materials", []):
			if not ids.has(mn):
				errors.append("loop %s: unknown material %s" % [ln, mn])
				continue
			_loop_mask[ids[mn]] = 1
			if not _loop_mats.has(ids[mn]):
				_loop_mats[ids[mn]] = []
			_loop_mats[ids[mn]].append(ln)
	return errors.is_empty()


func _check_sound(where: String, n: String) -> void:
	if not _sounds.has(n):
		errors.append("%s: unknown sound %s" % [where, n])


## A sound's definition (metres) -> what play() needs (voxels, linear ranges).
func _compile(n: String, d: Dictionary) -> void:
	var asset: String = d.get("asset", n)
	if not _index.has(asset):
		errors.append("sound %s: no asset %s" % [n, asset])
		return
	var p: Array = d.get("pitch", [0.95, 1.05])
	var bus: String = d.get("bus", "music" if asset.begins_with("music/") else "world")
	if not (bus in ["world", "ui", "music"]):
		errors.append("sound %s: unknown bus %s" % [n, bus])
		bus = "world"
	_sounds[n] = {"files": _index[asset], "db": float(d.get("db", -10.0)), "pitch": [float(p[0]), float(p[1])],
		"bus": {"world": "World", "ui": "UI", "music": "Music"}[bus], "flat": bool(d.get("flat", bus != "world")),
		"range_v": Settings.m2v(float(d.get("range_m", 24.0))), "ref_v": Settings.m2v(float(d.get("ref_m", 2.0))),
		"cooldown_ms": int(float(d.get("cooldown_s", 0.0)) * 1000.0)}


## The sound a material makes for a group event (step, hit, break, place, land), compiled once.
func material_sound(event: String, material: int) -> String:
	if material <= 0 or material >= _group_of.size():
		return ""
	var g := _group_of[material]
	var key := "%s:%s" % [event, g]
	if _sounds.has(key):
		return key
	var gd: Dictionary = _groups.get(g, {})
	var asset: String = gd.get("step" if event == "land" else ("break" if event == "place" else event), "")
	if asset == "":
		_sounds[key] = {}
		return ""
	var d: Dictionary = _events.get(event, {}).duplicate()
	d["asset"] = asset
	_compile(key, d)
	return key


func group_of(material: int) -> String:
	return _group_of[material] if material > 0 and material < _group_of.size() else "none"


func has_sound(n: String) -> bool:
	return _sounds.has(n) and not _sounds[n].is_empty()


func sound_names() -> Array:
	return _defs.keys()


# ---------------- playing ----------------

## Plays sound `n` at `pos` (world, voxel units), or flat without a position. opts: db (added),
## pitch (multiplied). Returns false when it was culled, cooling down or unknown.
func play(n: String, pos: Vector3 = Vector3.INF, opts: Dictionary = {}) -> bool:
	if not enabled or n == "":
		return false
	var s: Dictionary = _sounds.get(n, {})
	if s.is_empty():
		stats_data["missing"][n] = int(stats_data["missing"].get(n, 0)) + 1
		return false
	var now := Time.get_ticks_msec()
	if s["cooldown_ms"] > 0 and now - int(_last_play.get(n, -100000)) < s["cooldown_ms"]:
		stats_data["cooldown"] += 1
		return false
	var flat: bool = s["flat"] or pos == Vector3.INF
	if not flat:
		var ear := _listener()
		if ear != Vector3.INF and ear.distance_squared_to(pos) > s["range_v"] * s["range_v"]:
			stats_data["culled"] += 1
			return false
	var stream := _variant(n, s)
	if stream == null:
		return false
	_last_play[n] = now
	var db: float = s["db"] + float(opts.get("db", 0.0))
	var pitch: float = _rng.randf_range(s["pitch"][0], s["pitch"][1]) * float(opts.get("pitch", 1.0))
	if flat:
		var v := _voice2()
		v.stream = stream
		v.bus = s["bus"]
		v.volume_db = db
		v.pitch_scale = pitch
		v.play()
		_started[v] = now
	else:
		var v := _voice3()
		v.stream = stream
		v.bus = s["bus"]
		v.unit_size = s["ref_v"]
		v.max_distance = s["range_v"]
		v.volume_db = db
		v.pitch_scale = pitch
		v.global_position = pos
		v.play()
		_started[v] = now
	stats_data["played"] += 1
	stats_data["plays"][n] = int(stats_data["plays"].get(n, 0)) + 1
	var last: Array = stats_data["last"]
	last.append([snappedf(now / 1000.0, 0.01), n] if flat else [snappedf(now / 1000.0, 0.01), n, (pos / Settings.m2v(1.0)).snapped(Vector3.ONE * 0.1)])
	if last.size() > 24:
		last.pop_front()
	return true


## A material's group event at a voxel's centre.
func play_material(event: String, material: int, voxel: Vector3i, opts: Dictionary = {}) -> bool:
	return play(material_sound(event, material), Vector3(voxel) + Vector3(0.5, 0.5, 0.5), opts)


func _variant(n: String, s: Dictionary) -> AudioStream:
	var files: Array = s["files"]
	var i := _rng.randi() % files.size()
	if files.size() > 1 and i == int(_last_variant.get(n, -1)):
		i = (i + 1) % files.size()
	_last_variant[n] = i
	return _stream(files[i])


## Loads the files of every sound whose name starts with one of `prefixes` now, so a first play never
## reads disk mid-game: the interface ones at start-up, the rest under the loading screen (attach).
## Synchronous on purpose: threaded loads beside the game's own start-up broke the headless renderer.
func _preload(prefixes: Array) -> void:
	var t0 := Time.get_ticks_usec()
	for n in _index:
		if not n.begins_with("music/") and prefixes.any(func(p: String) -> bool: return n.begins_with(p)):
			for f in _index[n]:
				_stream(f)
	stats_data["preload_ms"] = snappedf(float(stats_data.get("preload_ms", 0.0)) + (Time.get_ticks_usec() - t0) / 1000.0, 0.1)


func _stream(file: String) -> AudioStream:
	if not _streams.has(file):
		var st = load(ROOT + file)
		if st == null:
			stats_data["missing"][file] = 1
		_streams[file] = st
	return _streams[file]


func _voice3() -> AudioStreamPlayer3D:
	var oldest: AudioStreamPlayer3D = _voices3[0]
	for v in _voices3:
		if not v.playing:
			return v
		if int(_started.get(v, 0)) < int(_started.get(oldest, 0)):
			oldest = v
	stats_data["stolen"] += 1
	return oldest


func _voice2() -> AudioStreamPlayer:
	var oldest: AudioStreamPlayer = _voices2[0]
	for v in _voices2:
		if not v.playing:
			return v
		if int(_started.get(v, 0)) < int(_started.get(oldest, 0)):
			oldest = v
	stats_data["stolen"] += 1
	return oldest


func _listener() -> Vector3:
	var cam := get_viewport().get_camera_3d()
	return cam.global_position if cam else Vector3.INF


func voices_busy() -> int:
	var n := 0
	for v in _voices3:
		n += 1 if v.playing else 0
	for v in _voices2:
		n += 1 if v.playing else 0
	return n


func stats() -> Dictionary:
	var s := stats_data.duplicate()
	s["busy"] = voices_busy()
	s["enabled"] = enabled
	return s


# ---------------- the UI ----------------

## Every button the UI adds gives one soft click; tabs, sliders and inventory slots stay silent (the
## developer's call after two rejected sets). Nothing in the UI code knows about sound.
func _on_node_added(n: Node) -> void:
	if n is BaseButton:
		if n is OptionButton:
			(n as OptionButton).item_selected.connect(func(_i: int) -> void: play("ui/click"))
		(n as BaseButton).pressed.connect(func() -> void: play("ui/click"))
	elif n is QuestBookView or n is RecipeBrowserView:
		var book := n as Control
		var was := [book.visible]
		book.visibility_changed.connect(func() -> void:
			if book.is_visible_in_tree() != was[0]:
				was[0] = book.is_visible_in_tree()
				play("book/open" if was[0] else "book/close"))
	elif n is Control and n.has_signal("pressed"):
		# drawn controls with their own pressed signal: the progression book's entries turn a page
		var argc := 0
		for d in n.get_signal_list():
			if d["name"] == "pressed":
				argc = d["args"].size()
		var cb := func() -> void: play("book/flip")
		n.connect("pressed", cb.unbind(argc) if argc > 0 else cb)


# ---------------- the game ----------------

## Main calls this once its systems exist. Everything is listened to; Main's leaving the tree detaches.
func attach(p_main: Node) -> void:
	main = p_main
	_player = main.get("player")
	main.tree_exiting.connect(_detach, CONNECT_ONE_SHOT)
	var p := _player
	p.voxel_placed.connect(func(at: Vector3i, m: int) -> void: play_material("place", m, at))
	p.voxels_placed.connect(func(cells: Array, m: int) -> void:
		if not cells.is_empty():
			play_material("place", m, cells[cells.size() / 2], {"db": 2.0 if cells.size() > 8 else 0.0}))
	p.landed.connect(_on_landed)
	p.died.connect(func() -> void: play("player/death"))
	p.inventory_toggled.connect(_on_inventory_toggled)
	p.block_used.connect(_on_block_used)
	var inv = main.get("player_inv")
	if inv:
		inv.tool_broke.connect(func(_id: String) -> void: play("player/tool_break"))
	var quests = main.get("quests")
	if quests:
		quests.quest_completed.connect(func(_id: String) -> void: play("ui/confirm"))
	var fx: ParticleFx = main.get("fx")
	if fx:
		for k in _particle_sounds.keys() + ["damage", "poof", "block_break", "block_hit", "sprint", "ember", "splash"]:
			fx.sound_bursts[k] = true
		fx.burst_played.connect(_on_burst)
	var pieces: Node = main.get("pieces")
	if pieces and pieces.has_signal("piece_event"):
		pieces.piece_event.connect(_on_piece_event)
	_picked = _picked_total()
	if enabled:
		_preload([""])
	_music_mode = "game"
	_fade_music_out()
	var first: Array = _music.get("first_s", [60, 180])
	_music_wait = _rng.randf_range(float(first[0]), float(first[1]))


func _detach() -> void:
	main = null
	_player = null
	for ln in _loops:
		(_loops[ln]["player"] as Node).queue_free()
	_loops.clear()
	_mob_next.clear()
	if _underwater_voice:
		_underwater_voice.stop()
	_set_underwater(false)
	_set_reverb(0.0)
	_music_mode = ""
	_fade_music_out()


func _loading() -> bool:
	var l = main.get("_loading")
	return l != null and is_instance_valid(l) and not l.is_done()


func _process(delta: float) -> void:
	var t0 := Time.get_ticks_usec()
	_update_music(delta)
	if main != null and is_instance_valid(main) and _player != null and enabled and not _loading():
		_watch_player(delta)
		_mob_t += delta
		if _mob_t >= 1.0 / MOB_HZ:
			_watch_mobs(_mob_t)
			_mob_t = 0.0
		_loop_t += delta
		if _loop_t >= 1.0 / LOOP_HZ:
			_sample_loops()
			_loop_t = 0.0
		_ease_loops(delta)
		_probe()
	var ms := (Time.get_ticks_usec() - t0) / 1000.0
	stats_data["ms_max"] = maxf(stats_data["ms_max"], ms)
	_ms_sum += ms
	_ms_frames += 1
	stats_data["ms_avg"] = snappedf(_ms_sum / _ms_frames, 0.001)


func _voxel(p: Vector3i) -> int:
	return int(main.get_world_voxel(p))


func _on_landed(feet: Vector3i, fall_m: float) -> void:
	if fall_m < 0.4:
		return
	var m := _voxel(feet)
	play_material("land", m, feet, {"db": clampf((fall_m - 1.0) * 2.0, -4.0, 6.0)})
	if fall_m > Settings.fall_damage_safe_m * 0.5:
		play("player/land", Vector3.INF, {"db": clampf(fall_m - 3.0, -6.0, 4.0)})


## Opening a chest or furnace does not send this (block_used rang it); closing always does.
func _on_inventory_toggled(open: bool) -> void:
	if not open and _chest_open:
		_chest_open = false
		play("block/chest_close", _player.position)
	else:
		play("ui/inventory")


func _on_block_used(p: Vector3i, m: int) -> void:
	var kind: String = GameMaterials.defs()[m - 1].get("interact", "") if m > 0 else ""
	var at := Vector3(p) + Vector3(0.5, 0.5, 0.5)
	match kind:
		"chest":
			_chest_open = true
			play("block/chest_open", at)
		"furnace":
			_chest_open = true
			play("block/latch", at)
		"crafting_table":
			play("craft/grid")


## ParticleFx bursts that tell a story (only the names in fx.sound_bursts are sent).
func _on_burst(n: String, pos: Vector3, opts: Dictionary) -> void:
	var mat := int(opts.get("material", 0))
	var near_player := _player.position.distance_to(pos) < Settings.m2v(2.2)
	var mining: bool = _player.mining_target().x != 1 << 30
	match n:
		"block_break":
			if mat == GameMaterials.WOOL and not near_player and _mob_near(pos, 1.5) != null:
				play("craft/shear", pos)
			play_material("break", mat, Vector3i(pos.floor()))
		"block_hit":
			if mining:
				return   # the player's own mining hits are timed in _watch_player
			match mat:
				GameMaterials.FLINT_STONES:
					play("craft/knap", pos)
				GameMaterials.CLAY:
					play("craft/clay", pos)
				GameMaterials.STONE:
					play("craft/anvil", pos)
				GameMaterials.DIRT:
					play("craft/dig_pit", pos)
				_:
					play_material("hit", mat, Vector3i(pos.floor()))
		"sprint":
			# the player's sprint dust is at its feet; elsewhere it is a sieve being shaken
			if _player.position.distance_to(pos) > Settings.m2v(1.5):
				play("craft/sieve", pos)
		"damage":
			if _player.camera.global_position.distance_to(pos) < Settings.m2v(1.5):
				return   # the player: the bleed burst says it
			var mob = _mob_near(pos, 2.0)
			if mob != null and _mobs.has(mob.species):
				play(_mobs[mob.species]["hurt"], pos)
			play("mob/hit", pos)
		"poof":
			if _player.camera.global_position.distance_to(pos) < Settings.m2v(1.2):
				return   # a tool breaking in the hand: tool_broke says it
			var mob = _mob_near(pos, 2.5)
			if mob != null:
				# its own hurt call, lower, and the body dropping
				if _mobs.has(mob.species):
					play(_mobs[mob.species]["hurt"], pos, {"pitch": 0.75})
				play("mob/death", pos)
			else:
				play("craft/done", pos)
		"splash":
			var speed := float(opts.get("speed_ms", 2.0))
			play("player/splash_big" if speed > 8.0 else ("player/splash" if speed > 3.0 else "player/splash_small"), pos)
		"ember":
			pass   # firepit embers and anvil sparks: the anvil's stone hit already rang
		_:
			play(_particle_sounds.get(n, ""), pos)


func _mob_near(pos: Vector3, radius_m: float):
	var mobs: Node = main.get("mobs")
	if mobs == null:
		return null
	var best = null
	var best_d := Settings.m2v(radius_m)
	for m in mobs.active():
		var d: float = m.eye().distance_to(pos)
		if d < best_d:
			best_d = d
			best = m
	return best


## Rigid pieces (P12): a trunk starts to go, hits the ground, settles; crowns shed leaves.
func _on_piece_event(kind: String, pos: Vector3, size: int) -> void:
	var big := size > 2000
	match kind:
		"spawned":
			if size > 300:
				play("tree/creak", pos, {"db": 2.0 if big else -2.0})
			else:
				play_material("break", GameMaterials.WOOD, Vector3i(pos.floor()))
		"impact":
			play("tree/crash" if big else "tree/thud", pos, {"db": clampf(log(maxf(size, 1)) - 7.0, -8.0, 3.0)})
		"breaks":
			play("tree/leaves", pos)


# ---------------- watching the player ----------------

func _watch_player(delta: float) -> void:
	var p := _player
	var feet: Vector3 = p.position
	var flat := Vector2(p.velocity.x, p.velocity.z).length()
	# steps: Minecraft steps about every block and a half walked; our stride is metres
	if p.is_on_floor() and not p.flying and not p.in_liquid and flat > Settings.m2v(0.5):
		var sprint: bool = flat > Settings.m2v((Settings.walk_speed_m + Settings.sprint_speed_m) * 0.5)
		_step_accum += flat * delta
		var stride := Settings.m2v(1.6 if sprint else 1.25)
		if _step_accum >= stride:
			_step_accum -= stride
			_step(feet, 2.0 if sprint else 0.0)
	else:
		_step_accum = Settings.m2v(1.0)   # the first step comes soon after starting to walk
	# swimming strokes and splashes
	if p.in_liquid and not p.flying:
		_swim_accum += p.velocity.length() * delta
		if _swim_accum >= Settings.m2v(1.4):
			_swim_accum = 0.0
			play("player/swim")
	if p.in_liquid and not _was_in_liquid and fx_missing():
		var speed_ms := Settings.v2m(maxf(-_last_vel.y, 0.0))
		if speed_ms > 1.0:
			play("player/splash_big" if speed_ms > 8.0 else "player/splash", feet)
	_was_in_liquid = p.in_liquid
	_last_vel = p.velocity
	# the head under water: muffled world, the underwater bed, breath bubbles
	var eye: Vector3 = p.camera.global_position
	var em := _voxel(Vector3i(eye.floor()))
	var under := group_of(em) == "water"
	if under != stats_data["underwater"]:
		_set_underwater(under)
	if under:
		_bubble_t -= delta
		if _bubble_t <= 0.0:
			_bubble_t = _rng.randf_range(1.5, 4.0)
			play("player/bubbles")
	# mining hits, four a second like Minecraft's dig sound
	var target: Vector3i = p.mining_target()
	if target.x != 1 << 30:
		_hit_t -= delta
		if _hit_t <= 0.0:
			_hit_t = 0.25
			play_material("hit", _voxel(target), target)
	else:
		_hit_t = 0.0
	if p.eating_item() != "":
		play("player/eat")
	# pickups (Main counts them; partial pickups count too)
	_picked_t += delta
	if _picked_t >= 0.1:
		_picked_t = 0.0
		var n := _picked_total()
		if n > _picked:
			play("player/pickup")
		_picked = n


## ParticleFx owns the splash on entering water when it exists (its burst carries the speed).
func fx_missing() -> bool:
	return main.get("fx") == null


func _step(feet: Vector3, db: float) -> void:
	var at := Vector3i(feet.floor())
	var m := _voxel(at)   # a thin layer (leaf litter, snow, plants, shallow water) at the feet sounds first
	var g := group_of(m)
	if not (g in ["plant", "leaves", "twig", "snow", "water"]):
		at = Vector3i((feet + Vector3(0, -0.1, 0)).floor())
		m = _voxel(at)
	play(material_sound("step", m), feet, {"db": db})


func _picked_total() -> int:
	var n := 0
	var d = main.get("picked_up_total")
	if d is Dictionary:
		for k in d:
			n += int(d[k])
	return n


func _set_underwater(on: bool) -> void:
	stats_data["underwater"] = on
	var w: int = _bus["World"]
	AudioServer.set_bus_effect_enabled(w, 0, on)
	(AudioServer.get_bus_effect(w, 0) as AudioEffectLowPassFilter).cutoff_hz = 700.0 if on else 20000.0
	if on and enabled and has_sound("loop/underwater"):
		if _underwater_voice == null:
			_underwater_voice = AudioStreamPlayer.new()
			_underwater_voice.name = "Underwater"
			_underwater_voice.bus = "Master"   # not muffled again by World's low-pass
			add_child(_underwater_voice)
		var s: Dictionary = _sounds["loop/underwater"]
		var st := _stream(s["files"][0])
		if st is AudioStreamOggVorbis:
			(st as AudioStreamOggVorbis).loop = true
		_underwater_voice.stream = st
		_underwater_voice.volume_db = s["db"] + linear_to_db(maxf(Settings.volume_effects, 0.0001))
		_underwater_voice.play()
	elif _underwater_voice:
		_underwater_voice.stop()


# ---------------- enclosure: cave reverb ----------------

## One ray per frame from the eye; the reverb follows how many directions are walled in near by
## (a roof counts double).
func _probe() -> void:
	var eye: Vector3 = _player.camera.global_position
	var dir: Vector3 = PROBE_DIRS[_probe_i].normalized()
	var hit := PROBE_RANGE_M
	var step := 2.0
	var t := step
	var max_v := Settings.m2v(PROBE_RANGE_M)
	while t <= max_v:
		var m := _voxel(Vector3i((eye + dir * t).floor()))
		if group_of(m) in ENCLOSING:
			hit = Settings.v2m(t)
			break
		t += step * (1.0 + t / 32.0)   # coarser further out
	_probe_hits[_probe_i] = hit
	_probe_i = (_probe_i + 1) % PROBE_DIRS.size()
	var closed := 0.0
	var weight := 0.0
	for i in PROBE_DIRS.size():
		var w := 2.0 if i == 0 else 1.0
		weight += w
		closed += w * (1.0 - clampf(_probe_hits[i] / PROBE_RANGE_M, 0.0, 1.0)) if _probe_hits[i] < PROBE_RANGE_M else 0.0
	var roofed := _probe_hits[0] < PROBE_RANGE_M
	var target := clampf(closed / weight * 1.6, 0.0, 1.0) * (1.0 if roofed else 0.3)
	_reverb = lerpf(_reverb, target, 0.05)
	_set_reverb(_reverb)


func _set_reverb(v: float) -> void:
	stats_data["reverb"] = snappedf(v, 0.01)
	var w: int = _bus["World"]
	var rv := AudioServer.get_bus_effect(w, 1) as AudioEffectReverb
	rv.wet = v * 0.45
	rv.room_size = 0.4 + 0.4 * v
	AudioServer.set_bus_effect_enabled(w, 1, v > 0.02)


# ---------------- animals ----------------

func _watch_mobs(dt: float) -> void:
	var mobs: Node = main.get("mobs")
	if mobs == null:
		return
	var ear := _listener()
	var seen := {}
	for m in mobs.active():
		var def: Dictionary = _mobs.get(m.species, {})
		if def.is_empty() or m.dying > 0.0:
			continue
		seen[m.uid] = true
		var every: Array = def.get("every_s", [10, 30])
		var angry: bool = m.goal in ["hunt", "attack"] and def.has("angry")
		var left: float = _mob_next.get(m.uid, _rng.randf_range(0.0, float(every[1])))
		left -= dt
		if left <= 0.0:
			var k := 0.3 if angry else 1.0   # a hunting bear roars more often, not constantly
			left = _rng.randf_range(float(every[0]) * k, float(every[1]) * k)
			var idle = def["idle"]
			var n: String = idle[_rng.randi() % idle.size()] if idle is Array else idle
			if angry:
				n = def["angry"]
			elif m.goal == "panic":
				n = def["hurt"]
			elif m.is_baby() and def.has("baby"):
				n = def["baby"]
			if ear == Vector3.INF or ear.distance_to(m.eye()) < Settings.m2v(48.0):
				play(n, m.eye())
		_mob_next[m.uid] = left
	for uid in _mob_next.keys():
		if not seen.has(uid):
			_mob_next.erase(uid)


# ---------------- sim loops ----------------

## Random cells around the camera with the loops' material mask; per loop the hits give a level
## (hits / full_hits) and a centre that its one looping voice follows.
func _sample_loops() -> void:
	var bubble: Node = main.get("bubble")
	if bubble == null or _loop_defs.is_empty():
		return
	var cam := _listener()
	if cam == Vector3.INF:
		return
	_loop_seed += 1
	var hits: PackedInt32Array = bubble.sample_cells(Vector3i(cam.floor()), int(Settings.m2v(LOOP_RADIUS_M)), LOOP_SAMPLES, _loop_seed, _loop_mask)
	if hits.is_empty() and not bubble.can_sample():
		return   # a tick is running: next round
	var acc := {}
	for i in range(0, hits.size(), 7):
		var m := hits[i + 3]
		var names: Array = _loop_mats.get(m, [])
		if hits[i + 6] & 1 and _loop_defs.has("fire"):   # SimWorld::FLAG_BURNING
			names = names + ["fire"]
		for ln in names:
			var l: Dictionary = _loop_defs[ln]
			match l.get("where", "any"):
				"air_above":
					if hits[i + 4] != GameMaterials.AIR:
						continue
				"air_below":
					if hits[i + 5] != GameMaterials.AIR:
						continue
				"moving":   # the cell moved this or the last tick: every voxel in motion murmurs
					if not (hits[i + 6] & 8):
						continue
				"fresh":    # a gas born this instant: steam off lava sizzles
					if not (hits[i + 6] & 16):
						continue
			var a: Array = acc.get(ln, [0, Vector3.ZERO])
			a[0] += 1
			a[1] += Vector3(hits[i], hits[i + 1], hits[i + 2])
			acc[ln] = a
	# lit firepits and pit kilns are block entities, not cells: the nearest feeds the embers loop
	var blocks = main.get("blocks")
	if blocks and _loop_defs.has("embers"):
		var best := Settings.m2v(LOOP_RADIUS_M)
		for e in blocks.all_work():
			if e.has_method("is_lit") and e.is_lit() and not (e is SieveEntity):
				var at := Vector3(e.pos) + Vector3(0.5, 0.5, 0.5)
				var d := at.distance_to(cam)
				if d < best:
					best = d
					acc["embers"] = [int(_loop_defs["embers"].get("full_hits", 2)), at * int(_loop_defs["embers"].get("full_hits", 2))]
	for ln in _loop_defs:
		var a: Array = acc.get(ln, [0, Vector3.ZERO])
		var lv := clampf(float(a[0]) / float(_loop_defs[ln].get("full_hits", 20)), 0.0, 1.0)
		var st: Dictionary = _loops.get(ln, {})
		if st.is_empty():
			if lv <= 0.0:
				continue
			st = _make_loop(ln)
		st["target"] = sqrt(lv)   # loudness grows slower than the cell count
		if a[0] > 0:
			st["pos"] = a[1] / float(a[0]) + Vector3(0.5, 0.5, 0.5)
		stats_data["loops"][ln] = snappedf(st["target"], 0.01)


func _make_loop(ln: String) -> Dictionary:
	var s: Dictionary = _sounds.get(_loop_defs[ln]["sound"], {})
	var v := AudioStreamPlayer3D.new()
	v.name = "Loop_" + ln
	v.bus = "World"
	v.doppler_tracking = AudioStreamPlayer3D.DOPPLER_TRACKING_DISABLED
	v.unit_size = s.get("ref_v", 16.0)
	v.max_distance = s.get("range_v", 192.0)
	var st := _stream(s["files"][0]) if not s.is_empty() else null
	if st is AudioStreamOggVorbis:
		(st as AudioStreamOggVorbis).loop = true
	v.stream = st
	v.volume_db = -80.0
	v.pitch_scale = float(_loop_defs[ln].get("pitch", 1.0))   # one asset serves several materials at different pitches
	add_child(v)
	var d := {"player": v, "level": 0.0, "target": 0.0, "pos": _listener(), "db": float(_loop_defs[ln].get("db", s.get("db", -10.0)))}
	_loops[ln] = d
	return d


func _ease_loops(delta: float) -> void:
	for ln in _loops.keys():
		var st: Dictionary = _loops[ln]
		var v: AudioStreamPlayer3D = st["player"]
		st["level"] = move_toward(st["level"], st["target"], delta * (2.0 if st["target"] > st["level"] else 0.6))
		v.global_position = v.global_position.lerp(st["pos"], minf(1.0, delta * 3.0)) if v.playing else st["pos"]
		if st["level"] > 0.01:
			v.volume_db = st["db"] + linear_to_db(st["level"])
			if not v.playing:
				v.play(_rng.randf() * maxf(0.0, v.stream.get_length() - 1.0) if v.stream else 0.0)
		elif v.playing:
			v.stop()
			stats_data["loops"].erase(ln)


# ---------------- music ----------------

func _update_music(delta: float) -> void:
	if not music_enabled or not enabled:
		return
	if main == null:
		var scene := get_tree().current_scene
		var want := "title" if scene is TitleScreen else ""
		if want != _music_mode:
			_music_mode = want
			_music_wait = 1.0 if want == "title" else -1.0
			if want == "":
				_fade_music_out()
	elif _loading():
		return
	if _music_wait < 0.0 or _music_player.playing:
		return
	_music_wait -= delta
	if _music_wait <= 0.0:
		_music_wait = -1.0
		play_music(_pick_track())


func _pick_track() -> String:
	if _music_mode == "title":
		return _pick(_music.get("title", []))
	var night := false
	var atm: Node = main.get("atmosphere") if main else null
	if atm:
		var t: float = atm.time_of_day
		night = t > atm.calendar.sunset() + 0.02 or t < atm.calendar.sunrise() - 0.02
	return _pick(_music.get("night" if night else "day", []))


func _pick(pool: Array) -> String:
	if pool.is_empty():
		return ""
	var n: String = pool[_rng.randi() % pool.size()]
	if pool.size() > 1 and n == stats_data["music"]:
		n = pool[(pool.find(n) + 1) % pool.size()]
	return n


## Starts a music asset now (fading in); "" stops.
func play_music(n: String) -> void:
	if n == "" or not _index.has(n):
		_fade_music_out()
		return
	var st := _stream(_index[n][0])
	if st == null:
		return
	if st is AudioStreamOggVorbis:
		(st as AudioStreamOggVorbis).loop = false
	stats_data["music"] = n
	_music_player.stream = st
	_music_player.volume_db = -40.0
	_music_player.play()
	if _music_tween:
		_music_tween.kill()
	_music_tween = create_tween()
	_music_tween.tween_property(_music_player, "volume_db", float(_music.get("db", -8.0)), float(_music.get("fade_s", 3.0)))


func next_music() -> String:
	play_music(_pick_track())
	return stats_data["music"]


func _fade_music_out() -> void:
	if not _music_player.playing:
		return
	if _music_tween:
		_music_tween.kill()
	_music_tween = create_tween()
	_music_tween.tween_property(_music_player, "volume_db", -40.0, float(_music.get("fade_s", 3.0)))
	_music_tween.tween_callback(_music_player.stop)
	stats_data["music"] = ""


func stop_music() -> void:
	_fade_music_out()
	_schedule_gap()


func _on_music_finished() -> void:
	stats_data["music"] = ""
	_schedule_gap()


func _schedule_gap() -> void:
	if _music_mode == "title":
		_music_wait = 60.0
	elif _music_mode == "game":
		var gap: Array = _music.get("gap_s", [300, 720])
		_music_wait = _rng.randf_range(float(gap[0]), float(gap[1]))
