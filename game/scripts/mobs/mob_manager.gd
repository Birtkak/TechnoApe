class_name MobManager
extends Node3D

## Every passive animal (ADR 0022), data in data/mobs.json.
##   spawning: the world is cut into 16 m cells; the first time a cell's ground
##     is loaded within 48 m of the player it rolls once (deterministic per seed
##     and cell, 10 % like a Minecraft chunk) for a herd of 2-4 of a species
##     whose biomes match the cell centre (spawn weights as in Minecraft); the
##     cell is then marked so herds never respawn. Passive animals never
##     despawn, as in Minecraft.
##   dormancy: an animal farther than 64 m from the player is stored as data
##     (no node, no cost) and comes back when the player returns; its age and
##     breeding timers advance by the time it slept.
##   interaction: aim (pick), attack, feed (love, babies grow), shear,
##     tempting players, breeding and the hearts above animals in love.
## Saved as JSON next to the world (Main writes it with the block entities).

const SAVE_VERSION := 1
const SEARCHES_PER_FRAME := 2   # walk-target searches (~100 voxel reads each) allowed per physics frame
const LIGHT_SAFE_M := 16.0       # night creatures never spawn this close to a torch or a burning station
const LIGHT_FADE_M := 6.0        # ... and one that comes this close fades like at dawn

var world: World
var main: Node                 # get_world_voxel / set_world_voxel / spawn_item / player / player_inv
var defs := {}                 # species -> definition
var errors: Array[String] = []
var cell_v := 32
var chance := 0.1
var spawn_radius_v := 96.0
var active_radius_v := 128.0
var max_active := 60
var night_max := 6             # night creatures about at once (ADR 0120)
var night_chance := 0.06       # a second, while under night_max
var night_now := false         # is_night() of the last scan: grazers sleep (Mob._think_goal)
var love_s := 30.0
var breed_cooldown_s := 300.0
var grow_up_s := 1200.0
var partner_radius_v := 16.0
var mate_s := 3.0
var feed_baby_speedup := 0.1
var spawning := true
var game_time := 0.0           # seconds this world has run (dormant mobs catch up from it)
var stats := {"spawned": 0, "herds": 0, "bred": 0, "died": 0, "slept": 0, "woke": 0, "cells": 0, "kills": 0, "mauled": 0, "eggs": 0, "carcasses": 0, "night_spawned": 0, "faded": 0, "alarmed": 0, "lit_away": 0, "warned": 0, "teleported": 0}
var dirty := false
var last_frame_ms := 0.0
var worst_frame_ms := 0.0
var _cost_sum := 0.0
var _cost_frames := 0
var frame_us := 0             # physics time of the manager and every mob since the last frame (Mob adds its own)
var part_us := PackedInt64Array([0, 0, 0, 0])   # think / move / animate / collide (VoxelBody, inside move) of every mob, summed since setup (JSON: per-step means)
var _steps := 0               # physics steps since setup
var collides := 0             # VoxelBody moves since setup

var _active: Array[Mob] = []
var _dormant: Array = []       # Mob.to_dict() + "slept_at"
var _populated := {}           # Vector2i cell -> true
var _next_uid := 1
var _scan_t := 0.0
var _hearts_t := {}            # uid -> seconds to the next heart
var _rng := RandomNumberGenerator.new()
var _owners := {}              # companion owner id -> Node3D (ADR 0152; "player" is Main's player)
var owner_foe := {}            # owner id -> [Mob, game_time]: what hurt the owner or what the owner hit last
var _seed := 0
var _searches := SEARCHES_PER_FRAME
const GRID_V := 2 * 2 * Settings.BLOCK   # neighbour grid cell: 2 m
var _grid := {}                # Vector2i cell -> Array of active mobs (neighbours())
var _grid_frame := -1


## A mob may run one walk-target search this frame (spreads the voxel reads of many brains).
func take_search() -> bool:
	if _searches <= 0:
		return false
	_searches -= 1
	return true


static func load_defs(path: String = "res://data/mobs.json") -> Dictionary:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		return {}
	var d = JSON.parse_string(f.get_as_text())
	return d if d is Dictionary else {}


func setup(p_world: World, p_main: Node, world_seed: int) -> void:
	world = p_world
	main = p_main
	_seed = world_seed
	_rng.randomize()
	configure(load_defs())
	MobModel.prewarm()
	for k in FurnitureEntity.KINDS:
		FurnitureEntity.mesh_for(k)
	_prewarm_fx.call_deferred()


## A pig and a window in front of the camera while the loading screen is up: their
## first draw compiles pipelines (a 180 ms frame otherwise). ParticleFx warms its own.
func _prewarm_fx() -> void:
	var cam: Camera3D = main.player.camera if main != null and main.player != null else null
	if cam == null:
		return
	var at := cam.global_position - cam.global_transform.basis.z * Settings.m2v(1.5)
	var warm := Node3D.new()
	warm.position = at
	add_child(warm)
	warm.add_child(MobModel.create("pig", false))
	warm.add_child(MobModel.create("chicken", false))
	warm.add_child(MobModel.create("bear", false))
	warm.add_child(FurnitureEntity.new(Vector3i.ZERO, "window", 0).make_view())   # the furniture palette and window glass
	get_tree().create_timer(0.5).timeout.connect(warm.queue_free)


func configure(data: Dictionary) -> void:
	if not data.has("mobs"):
		errors.append("mobs.json: expected {mobs: {...}}")
		return
	defs = data["mobs"]
	var sp: Dictionary = data.get("spawn", {})
	cell_v = int(Settings.m2v(float(sp.get("cell_m", 16.0))))
	chance = float(sp.get("chance", 0.1))
	spawn_radius_v = Settings.m2v(float(sp.get("spawn_radius_m", 48.0)))
	active_radius_v = Settings.m2v(float(sp.get("active_radius_m", 64.0)))
	max_active = int(sp.get("max_active", 60))
	night_max = int(sp.get("night_max", 6))
	night_chance = float(sp.get("night_chance", 0.06))
	var br: Dictionary = data.get("breeding", {})
	love_s = float(br.get("love_s", 30.0))
	breed_cooldown_s = float(br.get("breed_cooldown_s", 300.0))
	grow_up_s = float(br.get("grow_up_s", 1200.0))
	partner_radius_v = Settings.m2v(float(br.get("partner_radius_m", 8.0)))
	mate_s = float(br.get("mate_s", 3.0))
	feed_baby_speedup = float(br.get("feed_baby_speedup", 0.1))
	for k in defs:
		for key in ["health", "size_m", "food", "biomes"]:
			if not defs[k].has(key):
				errors.append("mobs.json: %s needs %s" % [k, key])


# ---------------- per frame ----------------

func _physics_process(delta: float) -> void:
	var t0 := Time.get_ticks_usec()
	_searches = SEARCHES_PER_FRAME
	if not _active.is_empty():
		_steps += 1
	game_time += delta
	_scan_t -= delta
	if _scan_t <= 0.0:
		_scan_t = 1.0
		_update_dormancy()
		if spawning:
			_update_spawning()
		_night_tick()
	frame_us += Time.get_ticks_usec() - t0


## Mob physics this frame into Main.phys_cost (DebugCapture lists it on a spike).
func _process(_delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(_delta)
	DebugCapture.note_proc("mobs", _pt0)


func _process_timed(_delta: float) -> void:
	if main != null and "phys_cost" in main:
		main.phys_cost["mobs"] = maxf(float(main.phys_cost.get("mobs", 0.0)), frame_us / 1000.0)
	last_frame_ms = frame_us / 1000.0
	worst_frame_ms = maxf(worst_frame_ms, last_frame_ms)
	_cost_sum += last_frame_ms
	_cost_frames += 1
	frame_us = 0


func active() -> Array[Mob]:
	return _active


func count() -> int:
	return _active.size() + _dormant.size()


func spawn(species: String, pos: Vector3, baby: bool = false, yaw: float = 0.0) -> Mob:
	if not defs.has(species):
		return null
	var m := Mob.create(self, species, defs[species], baby)
	m.uid = _next_uid
	_next_uid += 1
	m.position = pos
	m.rotation.y = yaw
	add_child(m)
	_active.append(m)
	stats["spawned"] += 1
	dirty = true
	return m


func _player_pos() -> Vector3:
	return main.player.position if main != null and main.player != null else Vector3.ZERO


func player_eye() -> Vector3:
	return main.player.camera.global_position if main != null and main.player != null else Vector3.INF


func _update_dormancy() -> void:
	var pp := _player_pos()
	var keep: Array[Mob] = []
	for m in _active:
		if not is_instance_valid(m):
			continue
		if not m.is_dead() and m.owner_id == "" and Vector2(m.position.x - pp.x, m.position.z - pp.z).length() > active_radius_v:
			if m.def.get("night", false):   # night creatures are not kept: the next night brings others
				m.queue_free()
				continue
			var d := m.to_dict()
			d["slept_at"] = game_time
			_dormant.append(d)
			stats["slept"] += 1
			m.queue_free()
			continue
		keep.append(m)
	_active = keep
	for m in _active:
		if m.owner_id != "" and not m.sitting and not m.is_dead():   # left far behind between brain ticks (a teleport, a fast ride)
			var o := owner_node(m.owner_id)
			if o != null and o.position.distance_to(m.position) > Settings.m2v(float(m.def.get("companion", {}).get("teleport_m", 22.0))):
				teleport_companion(m, o)
	if _dormant.is_empty():
		return
	var still := []
	for d in _dormant:
		var p: Array = d["pos"]
		var own := owner_node(String(d.get("owner", ""))) if d.has("owner") else null
		if own != null and Vector2(float(p[0]) - own.position.x, float(p[2]) - own.position.z).length() > Settings.m2v(20.0):
			d["pos"] = [own.position.x, own.position.y, own.position.z]   # a companion saved far from its owner comes to them
			p = d["pos"]
		var near := Vector2(float(p[0]) - pp.x, float(p[2]) - pp.z).length() < active_radius_v - Settings.m2v(8.0)
		if near and _active.size() < max_active * 2 and world.is_area_loaded(Vector3i((Vector3(float(p[0]), float(p[1]), float(p[2])) / Settings.CHUNK).floor())):
			_wake(d)
		else:
			still.append(d)
	_dormant = still


func _wake(d: Dictionary) -> void:
	var slept: float = game_time - float(d.get("slept_at", game_time))
	var m := Mob.create(self, String(d["species"]), defs.get(String(d["species"]), {}), false)
	if m.def.is_empty():
		return
	m.load_from(d)
	m.age_s = minf(0.0, m.age_s + slept) if m.age_s < 0.0 else m.age_s
	m.love_s = maxf(0.0, m.love_s - slept)
	m.breed_cooldown_s = maxf(0.0, m.breed_cooldown_s - slept)
	add_child(m)
	_active.append(m)
	stats["woke"] += 1


# ---------------- spawning ----------------

func _update_spawning() -> void:
	if _active.size() >= max_active:
		return
	var pp := _player_pos()
	var r := int(ceil(spawn_radius_v / cell_v))
	var pc := Vector2i(floori(pp.x / cell_v), floori(pp.z / cell_v))
	for dx in range(-r, r + 1):
		for dz in range(-r, r + 1):
			var c := pc + Vector2i(dx, dz)
			if _populated.has(c):
				continue
			var centre := Vector2((c.x + 0.5) * cell_v, (c.y + 0.5) * cell_v)
			if centre.distance_to(Vector2(pp.x, pp.z)) > spawn_radius_v:
				continue
			var h := world.surface_height(int(centre.x), int(centre.y))
			if not world.is_area_loaded(Vector3i(floori(centre.x / Settings.CHUNK), floori(float(h) / Settings.CHUNK), floori(centre.y / Settings.CHUNK))):
				continue
			_populated[c] = true
			stats["cells"] += 1
			dirty = true
			_populate(c)


## One deterministic roll per cell: a herd of the species the cell centre's biome allows.
func _populate(c: Vector2i) -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = hash([_seed, c.x, c.y, 0x5EED])
	# seasons (ADR 0025): spring herds bring lambs and piglets, winter herds are fewer and smaller
	var season := _season()
	var herd_chance := chance * (0.5 if season == 0 else 1.0)
	var baby_chance := 0.4 if season == 1 else 0.1
	WildMoves.populate(self, c)   # fish, shore and cave groups roll on their own (ADR 0058)
	if rng.randf() >= herd_chance:
		return
	var cx := int((c.x + 0.5) * cell_v)
	var cz := int((c.y + 0.5) * cell_v)
	var biome := WorldGen.biome_name(int(world.generator.gen.column_info(cx, cz)["biome"]))
	var total := 0
	var options := []
	for k in defs:
		if biome in defs[k].get("biomes", []) and WildMoves.habitat(defs[k]) == "land" and not defs[k].get("night", false):
			options.append(k)
			total += int(defs[k].get("weight", 1))
	if total <= 0:
		return
	var pick := rng.randi_range(0, total - 1)
	var species: String = options[0]
	for k in options:
		pick -= int(defs[k].get("weight", 1))
		if pick < 0:
			species = k
			break
	if defs[species].has("rarity") and rng.randf() >= float(defs[species]["rarity"]):   # the roll only for rare ones: older herds stay where they were
		return   # rare species (bears): most of the cells that pick them stay empty
	var g: Array = defs[species].get("group", [2, 4])
	var n := rng.randi_range(int(g[0]), int(g[1]))
	if season == 0:
		n = maxi(1, n - 1)
	var placed := 0
	var height := int(ceil(Settings.m2v(float(defs[species]["size_m"][1]))))
	for _i in n * 4:
		if placed >= n:
			break
		var x := cx + rng.randi_range(-10, 10) * Settings.BLOCK
		var z := cz + rng.randi_range(-10, 10) * Settings.BLOCK
		var h := world.surface_height(x, z)
		var y := stand_height(x, z, h + 1, 4 * Settings.BLOCK, height)
		var ground := get_voxel(Vector3i(x, y - 1, z))
		if y == -1 or not WildMoves.ground_ok(defs[species], ground):
			continue
		spawn(species, Vector3(x + 0.5, y + 0.05, z + 0.5), rng.randf() < baby_chance, rng.randf() * TAU)
		placed += 1
	if placed > 0:
		stats["herds"] += 1


# ---------------- night creatures (ADR 0120) ----------------

## True between dusk and dawn by Main's calendar; false without a sky (tests, benches).
func is_night() -> bool:
	var atmo: Node = main.get("atmosphere") if main != null else null
	if atmo == null:
		return false
	var cal = atmo.calendar
	return GameAudio.day_phase(cal.time, cal.sunrise(), cal.sunset()) == "night"


## Once a second: at night a pack may come out 24-40 m from the player (up to spawn.night_max night
## creatures at once); after sunrise the ones still about fade into the light.
## Voxel positions of burning torches and stations (BlockEntities.lit): night creatures keep away.
func _lights() -> Array:
	var out := []
	var blocks: Variant = main.get("blocks") if main != null else null
	if blocks != null:
		for e in blocks.lit():
			out.append(Vector3(e.pos))
	return out


static func near_light(lights: Array, p: Vector3, r_m: float) -> bool:
	var r := Settings.m2v(r_m)
	return lights.any(func(l: Vector3) -> bool: return l.distance_to(p) < r)


func _night_tick() -> void:
	var night := is_night()
	night_now = night
	var n := 0
	var lights := _lights()
	for m in _active.duplicate():
		if not m.def.get("night", false) or m.is_dead():
			continue
		if night and not near_light(lights, m.position, LIGHT_FADE_M):
			n += 1
			continue
		_poof(m.eye())
		_active.erase(m)
		m.queue_free()
		stats["faded"] += 1
	if not night or not spawning or n >= night_max or _rng.randf() >= night_chance:
		return
	var pp := _player_pos()
	var a := _rng.randf() * TAU
	var d := Settings.m2v(_rng.randf_range(24.0, 40.0))
	var x := int(pp.x + cos(a) * d)
	var z := int(pp.z + sin(a) * d)
	var h := world.surface_height(x, z)
	if not world.is_area_loaded(Vector3i(floori(float(x) / Settings.CHUNK), floori(float(h) / Settings.CHUNK), floori(float(z) / Settings.CHUNK))):
		return
	if near_light(lights, Vector3(x, h, z), LIGHT_SAFE_M):
		stats["lit_away"] += 1
		return
	var biome := WorldGen.biome_name(int(world.generator.gen.column_info(x, z)["biome"]))
	var options := []
	for k in defs:
		if defs[k].get("night", false) and biome in defs[k].get("biomes", []):
			options.append(k)
	if options.is_empty():
		return
	var species: String = options[_rng.randi() % options.size()]
	var g: Array = defs[species].get("group", [1, 2])
	var height := int(ceil(Settings.m2v(float(defs[species]["size_m"][1]))))
	for i in mini(_rng.randi_range(int(g[0]), int(g[1])), night_max - n):
		var px := x + _rng.randi_range(-3, 3) * Settings.BLOCK
		var pz := z + _rng.randi_range(-3, 3) * Settings.BLOCK
		var y := stand_height(px, pz, world.surface_height(px, pz) + 1, 4 * Settings.BLOCK, height)
		if y == -1 or not WildMoves.ground_ok(defs[species], get_voxel(Vector3i(px, y - 1, pz))):
			continue
		spawn(species, Vector3(px + 0.5, y + 0.05, pz + 0.5), false, _rng.randf() * TAU)
		stats["night_spawned"] += 1


# ---------------- world queries (for Mob) ----------------

## The calendar's season (0 winter .. 3 autumn), or -1 without a sky (tests).
func _season() -> int:
	var atmo: Node = main.get("atmosphere") if main != null else null
	return atmo.calendar.season() if atmo != null else -1


func get_voxel(p: Vector3i) -> int:
	return main.get_world_voxel(p) if main != null else world.get_voxel(p)


static func _def_of(m: int) -> Dictionary:
	return GameMaterials.defs()[m - 1] if m > 0 else {}


## Blocks a body (VoxelBody's view): solids and powders that are not micro plants.
static func is_solid(m: int) -> bool:
	if m <= 0:
		return false
	var d := _def_of(m)
	var ct: String = d.get("cell_type", "solid")
	if (ct != "solid" and ct != "powder") or d.get("cross", false):
		return false
	return not d.has("micro") or d["micro"] in ["stalactite", "stalagmite"]


func is_liquid(p: Vector3i) -> bool:
	var m := GameMaterials.material_of_voxel(get_voxel(p))
	return m > 0 and (_def_of(m).get("cell_type", "solid") == "liquid" or GameMaterials.is_waterlogged(m))


## The lowest y in [start - v, start + v] where a body `height` voxels tall can
## stand at column (x, z): solid below, no solid or liquid in its height. -1 if none.
func stand_height(x: int, z: int, start: int, v: int, height: int) -> int:
	var best := -1
	for y in range(start + v, start - v - 1, -1):
		var below := get_voxel(Vector3i(x, y - 1, z))
		if not is_solid(below):
			continue
		var ok := true
		for k in height:
			var m := get_voxel(Vector3i(x, y + k, z))
			if is_solid(m) or (m > 0 and _def_of(m).get("cell_type", "") == "liquid"):
				ok = false
				break
		if ok:
			best = y
			if absi(y - start) <= 1:
				return y
		elif best != -1:
			return best
	return best


## Can the mob keep walking `dir`? No drop deeper than MAX_DROP_V and no
## liquid ahead (panicking animals still avoid lava and cliffs, not water).
func safe_ahead(m: Mob, dir: Vector3, panicking: bool) -> bool:
	var probe := m.position + dir * (m._half_v + 0.7 * Settings.BLOCK)
	var x := int(floor(probe.x))
	var z := int(floor(probe.z))
	var y := int(floor(m.position.y + 0.05))
	if m.owner_id != "" and main != null and main.get("fishing") != null and main.fishing.trap_near(probe):
		return false   # ADR 0163: a companion steps round snares and pit traps
	for k in range(0, Mob.MAX_DROP_V + 2):
		var p := Vector3i(x, y - k, z)
		var v := get_voxel(p)
		if GameMaterials.material_of_voxel(v) in [GameMaterials.LAVA, GameMaterials.LAVA_STATIC]:
			return false
		if GameMaterials.material_of_voxel(v) == GameMaterials.FOULED_WATER:
			return m.in_liquid   # ADR 0141: animals and fish keep out of fouled water (one already in it may swim out)
		if v > 0 and _def_of(v).get("cell_type", "") == "liquid":
			return panicking or m.in_liquid or WildMoves.water_ok(m)
		if is_solid(v):
			return k <= Mob.MAX_DROP_V + 1
	return false


## Standing on grass or in a tuft: something to graze.
func can_graze(m: Mob) -> bool:
	var feet := Vector3i(m.position.floor())
	var here := get_voxel(feet)
	return GameMaterials.is_tall_grass(here) or GameMaterials.is_grass(get_voxel(feet + Vector3i.DOWN))


## Eats the tuft at the feet (it becomes air). Grass blocks are grazed but kept:
## nothing regrows grass yet (ADR 0022). Returns true when it ate.
func graze(m: Mob) -> bool:
	var feet := Vector3i(m.position.floor())
	var fx := _fx()
	var tuft := get_voxel(feet)
	if GameMaterials.is_tall_grass(tuft):
		main.set_world_voxel(feet, GameMaterials.AIR)
		if fx:
			fx.block_break(feet, tuft)
		return true
	var ate := GameMaterials.is_grass(get_voxel(feet + Vector3i.DOWN))
	if ate and fx:
		fx.burst("graze", Vector3(m.eye().x, feet.y + 0.1, m.eye().z), {"material": GameMaterials.TALL_GRASS, "floor": float(feet.y)})
	return ate


## A landing on farmland may trample it (Minecraft: babies are too light).
func landed(m: Mob, fall_m: float) -> void:
	if fall_m > 0.5 and not m.is_baby() and main != null and main.has_method("trample"):
		main.trample(Vector3i((m.position + Vector3(0, -0.1, 0)).floor()), fall_m)
	if fall_m > 0.5 and _fx():
		_fx().landing(Vector3i((m.position + Vector3(0, -0.1, 0)).floor()), fall_m, m.position)


## Animals in the 3 x 3 grid cells (GRID_V voxels, wider than any two half-widths) around p.
## The grid is rebuilt at most once per physics frame: separate() ran over every animal for every
## brain, 10 000 pair tests a think with 100 bears.
func neighbours(p: Vector3) -> Array:
	var f := Engine.get_physics_frames()
	if f != _grid_frame:
		_grid_frame = f
		_grid.clear()
		for o in _active:
			var k := Vector2i(floori(o.position.x / GRID_V), floori(o.position.z / GRID_V))
			if _grid.has(k):
				_grid[k].append(o)
			else:
				_grid[k] = [o]
	var out := []
	var c := Vector2i(floori(p.x / GRID_V), floori(p.z / GRID_V))
	for dx in range(-1, 2):
		for dz in range(-1, 2):
			var a = _grid.get(c + Vector2i(dx, dz))
			if a != null:
				out.append_array(a)
	return out


## Pushes overlapping animals (and the player) apart, Minecraft's entity push.
func separate(m: Mob) -> void:
	for o: Mob in neighbours(m.position):
		if o == m or o.is_dead():
			continue
		var d := Vector2(m.position.x - o.position.x, m.position.z - o.position.z)
		var r := m._half_v + o._half_v
		if d.length() < r and absf(m.position.y - o.position.y) < 1.5 * Settings.BLOCK:
			var push := d.normalized() if d.length() > 0.01 else Vector2(1, 0)
			m.velocity.x += push.x * 0.8 * Settings.BLOCK
			m.velocity.z += push.y * 0.8 * Settings.BLOCK
	var pp := _player_pos()
	var dp := Vector2(m.position.x - pp.x, m.position.z - pp.z)
	if dp.length() < m._half_v + Settings.m2v(Settings.player_radius_m) and absf(pp.y - m.position.y) < m._height_v:
		var push := dp.normalized() if dp.length() > 0.01 else Vector2(0, 1)
		m.velocity.x += push.x * 1.2 * Settings.BLOCK
		m.velocity.z += push.y * 1.2 * Settings.BLOCK
		var venom: float = defs[m.species].get("poison_s", 0.0)   # a dart frog's skin poisons on touch
		if venom > 0.0 and main.get("survival") != null and player_attackable():
			main.survival.poison(venom)


# ---------------- social ----------------

## A hurt animal alarms its herd: the others of its kind within 12 m bolt too (a stampede, not one
## sheep at a time). Predators stand their ground.
func alarm(m: Mob) -> void:
	var r := Settings.m2v(12.0)
	for o in _active:
		if o != m and o.species == m.species and not o.is_dead() and o.position.distance_to(m.position) < r:
			o.scare()
			stats["alarmed"] += 1


## Position of a player holding this species' food within 10 m, or INF.
func tempt_target(m: Mob) -> Vector3:
	if main == null or main.player_inv == null or main.player.ui_open:
		return Vector3.INF
	var held: Dictionary = main.player_inv.held()
	if held.is_empty() or not (String(held["id"]) in m.def.get("food", [])):
		return Vector3.INF
	var pp := _player_pos()
	if pp.distance_to(m.position) > Settings.m2v(10.0):
		return Vector3.INF
	return pp


func find_partner(m: Mob) -> Mob:
	var best: Mob = null
	var best_d := partner_radius_v
	for o in _active:
		if o == m or o.species != m.species or o.is_baby() or o.love_s <= 0.0 or o.is_dead():
			continue
		var d := o.position.distance_to(m.position)
		if d < best_d:
			best_d = d
			best = o
	return best


func nearest_adult(m: Mob) -> Mob:
	var best: Mob = null
	var best_d := Settings.m2v(8.0)
	for o in _active:
		if o == m or o.species != m.species or o.is_baby() or o.is_dead():
			continue
		var d := o.position.distance_to(m.position)
		if d < best_d:
			best_d = d
			best = o
	return best


## Centre of the other animals of this species within 16 m, or INF.
func herd_centre(m: Mob) -> Vector3:
	var sum := Vector3.ZERO
	var n := 0
	for o in _active:
		if o == m or o.species != m.species:
			continue
		if o.position.distance_to(m.position) < Settings.m2v(16.0):
			sum += o.position
			n += 1
	return sum / n if n > 0 else Vector3.INF


func breed(a: Mob, b: Mob) -> Mob:
	a.love_s = 0.0
	b.love_s = 0.0
	a.breed_cooldown_s = breed_cooldown_s
	b.breed_cooldown_s = breed_cooldown_s
	a._mate_t = 0.0
	b._mate_t = 0.0
	var baby := spawn(a.species, (a.position + b.position) * 0.5 + Vector3(0, 0.1, 0), true, _rng.randf() * TAU)
	stats["bred"] += 1
	for i in 6:
		_heart((a.position + b.position) * 0.5 + Vector3(_rng.randf_range(-1, 1), 2.0 + _rng.randf(), _rng.randf_range(-1, 1)))
	return baby


func hearts_tick(m: Mob, delta: float) -> void:
	var t: float = _hearts_t.get(m.uid, 0.0) - delta
	if t <= 0.0:
		t = 0.6
		_heart(m.eye() + Vector3(_rng.randf_range(-0.6, 0.6), 0.8, _rng.randf_range(-0.6, 0.6)))
	_hearts_t[m.uid] = t


## The game's particles (ADR 0026); null without a Main (tests).
func _fx() -> ParticleFx:
	return main.get("fx") if main != null else null


func _heart(at: Vector3) -> void:
	if _fx():
		_fx().burst("heart", at)


func _poof(at: Vector3) -> void:
	if _fx():
		_fx().burst("poof", at)


# ---------------- player actions ----------------

## The animal under the aim ray within max_dist: {mob, dist} or {}.
func pick(from: Vector3, dir: Vector3, max_dist: float) -> Dictionary:
	var best := {}
	var to := from + dir * max_dist
	for m in _active:
		if m.is_dead():
			continue
		var hit = m.aabb().grow(0.05).intersects_segment(from, to)
		if hit != null:
			var d: float = from.distance_to(hit)
			if best.is_empty() or d < float(best["dist"]):
				best = {"mob": m, "dist": d}
	return best


func attack(m: Mob, damage: int, from: Vector3) -> bool:
	var by_player: bool = main != null and main.player != null and from == main.player.position
	if m.owner_id != "" and by_player:
		return false   # the owner's blows do not hurt their companion
	var hurt := m.hurt(damage, from)
	if hurt and by_player:
		owner_foe["player"] = [m, game_time]   # the companion joins in on whatever its owner hits
	if hurt and _fx():
		_fx().burst("damage", m.eye(), {"count_bonus": damage})   # Minecraft's damage indicator hearts
	return hurt


## Right click on an animal with `held` (item def). Returns what happened:
## "love" / "grew" (fed: take one food), "sheared" (shears), "milked" (takes the bowl), or "".
func interact(m: Mob, held: Dictionary) -> String:
	if m.is_dead():
		return ""
	if m.owner_id != "":
		if held.is_empty():
			return "sit" if m.toggle_sit() else "follow"   # ADR 0152: an empty hand tells a companion to stay or come
		if "meat" in held.get("tags", []) and m.eat_meat():
			return "grew"   # uses up the meat
		return ""
	if held.is_empty():
		return ""
	var id: String = held.get("id", "")
	if id in m.def.get("food", []):
		var r := m.feed()
		if r == "love":
			for i in 3:
				_heart(m.eye() + Vector3(_rng.randf_range(-0.5, 0.5), 0.6 + i * 0.3, _rng.randf_range(-0.5, 0.5)))
		return r
	if "shears" in held.get("tags", []) and m.def.has("shear") and not m.sheared and not m.is_baby():
		m.set_sheared(true)
		if _fx():
			_fx().burst("block_break", m.eye() + Vector3(0, -0.3, 0), {"material": GameMaterials.WOOL})
		for s in MiningTable.roll_list(m.def["shear"], _rng):
			main.spawn_item(s, m.eye(), Vector3(_rng.randf_range(-1.5, 1.5), 3.0, _rng.randf_range(-1.5, 1.5)))
		dirty = true
		return "sheared"
	var milk: Dictionary = m.def.get("milk", {})
	if id == milk.get("with", "") and not m.sheared and not m.is_baby():   # the sheared flag is "milked": it refills as the animal grazes
		m.set_sheared(true)
		main.spawn_item({"id": milk["gives"], "count": 1}, m.eye(), Vector3(0.0, 2.0, 0.0))
		dirty = true
		return "milked"
	return ""


# ---------------- companions (ADR 0152) ----------------

## The id a companion stores for its owner: "player" for the local player, else the node's name
## (a remote player's name; the companion is local to this client until mobs are shared, ADR 0145).
func owner_id_of(o: Node) -> String:
	if main != null and o == main.player:
		return "player"
	return String(o.get_meta("player_id", o.name))


func register_owner(id: String, o: Node3D) -> void:
	_owners[id] = o


## The owner's node, or null when they are not in this game (a remote player who left).
func owner_node(id: String) -> Node3D:
	if id == "player" and main != null and main.player != null:
		return main.player
	var o = _owners.get(id)
	return o if is_instance_valid(o) else null


func is_threat(o: Mob) -> bool:
	return not o.is_dead() and o.owner_id == "" and (o.def.get("night", false) or (o.def.has("predator") and (o.aggressive or o._provoked_t > 0.0)))


## What the companion fights: the owner's last foe (hit or hurt by, 10 s) or the nearest threat within r.
func companion_foe(m: Mob, owner_p: Vector3, r: float) -> Mob:
	var f = owner_foe.get(m.owner_id)
	if f != null and is_instance_valid(f[0]) and f[0] != m and not f[0].is_dead() and game_time - float(f[1]) < 10.0 \
			and f[0].position.distance_to(owner_p) < r * 2.0:
		return f[0]
	var best: Mob = null
	var best_d := r
	for o in _active:
		if o == m or not is_threat(o):
			continue
		var d := minf(o.position.distance_to(m.position), o.position.distance_to(owner_p))
		if d < best_d:
			best_d = d
			best = o
	return best


## Growl (sound + a line) at the nearest threat in sight, once per threat per 25 s.
func companion_warn(m: Mob, r: float) -> void:
	var best: Mob = null
	var best_d := r
	for o in _active:
		if not is_threat(o) or game_time - float(m._warned.get(o.uid, -1e9)) < 25.0:
			continue
		var d := o.position.distance_to(m.position)
		if d < best_d:
			best_d = d
			best = o
	if best == null:
		return
	m._warned[best.uid] = game_time
	m._look = best.eye()
	m._look_t = 2.0
	stats["warned"] += 1
	Sound.play("mob/wolf", m.eye())
	var work = main.get("work") if main != null else null
	if work != null:
		work._say("Your dog growls: %s." % String(best.def.get("name", best.species)).to_lower())


## Back to the owner: a standable spot 2-3 m from them (their own spot when none).
func teleport_companion(m: Mob, o: Node3D) -> void:
	var h := int(m._height_v) + 1
	var at := o.position
	for _i in 12:
		var a := _rng.randf() * TAU
		var r := Settings.m2v(_rng.randf_range(2.0, 3.0))
		var x := int(floor(o.position.x + cos(a) * r))
		var z := int(floor(o.position.z + sin(a) * r))
		var y := stand_height(x, z, int(floor(o.position.y)), 4 * Settings.BLOCK, h)
		if y != -1:
			at = Vector3(x + 0.5, y + 0.05, z + 0.5)
			break
	m.position = at
	m.velocity = Vector3.ZERO
	m._stop()
	m._fall_peak = at.y
	_poof(m.position + Vector3(0, 0.5, 0))
	stats["teleported"] += 1


func heart_at(at: Vector3) -> void:
	_heart(at)


# ---------------- wildlife (ADR 0027) ----------------

func player_attackable() -> bool:
	if main == null or main.player == null or main.get("player_inv") == null:
		return false
	return GameMode.rule(main.player.game_mode, "takes_damage") and not main.player_inv.is_dead()


func player_feet() -> Vector3:
	return _player_pos()


## The nearest live prey of a predator within its sight.
func find_prey(m: Mob) -> Mob:
	var pr: Dictionary = m.def.get("predator", {})
	var kinds: Array = pr.get("prey", [])
	var sight := Settings.m2v(float(pr.get("sight_m", 24.0)))
	var best: Mob = null
	var best_d := sight
	for o in _active:
		if o == m or o.is_dead() or not (o.species in kinds):
			continue
		var d := o.position.distance_to(m.position)
		if d < best_d:
			best_d = d
			best = o
	return best


## A predator bites its prey; a killing bite leaves the prey's carcass and feeds the predator.
func maul(m: Mob, prey: Mob, damage: int) -> void:
	if prey.hurt(damage, m.position):
		if _fx():
			_fx().burst("damage", prey.eye(), {"count_bonus": damage})
		if prey.is_dead():
			prey.killed_by = m.species
			stats["kills"] += 1
			m.on_kill(prey.position)


## A predator bites the player: damage and a shove away from it.
func maul_player(m: Mob, damage: int) -> void:
	if not player_attackable():
		return
	if main.stone_age and main.stone_age.clothing:
		damage = main.stone_age.clothing.armor_cut(damage)
	main.player_inv.damage(damage, "%s attack" % m.species)
	owner_foe["player"] = [m, game_time]   # the companion joins in on whatever hurts its owner
	var away: Vector3 = main.player.position - m.position
	away.y = 0.0
	if away.length() > 0.01:
		main.player.velocity += away.normalized() * Settings.m2v(5.0) + Vector3(0, Settings.m2v(3.0), 0)
	stats["mauled"] += 1
	if _fx():
		_fx().burst("damage", player_eye() + Vector3(0, -0.4, 0), {"count_bonus": damage})


## A chicken lays an egg where it stands.
func lay(m: Mob) -> void:
	var item: String = m.def.get("lays", {}).get("item", "egg")
	main.spawn_item({"id": item, "count": 1}, m.position + Vector3(0, 0.3, 0), Vector3(_rng.randf_range(-0.5, 0.5), 1.0, _rng.randf_range(-0.5, 0.5)))
	stats["eggs"] += 1


## Where a kill lies: the remains go on the first free voxel over solid ground.
func _leave_carcass(at: Vector3) -> void:
	var p := Vector3i(at.floor())
	for dy in [0, 1, -1]:
		var q: Vector3i = p + Vector3i(0, dy, 0)
		if get_voxel(q) == GameMaterials.AIR and is_solid(get_voxel(q + Vector3i.DOWN)):
			main.set_world_voxel(q, GameMaterials.CARCASS)
			stats["carcasses"] += 1
			return


func on_died(m: Mob) -> void:
	if m.killed_by != "":
		_leave_carcass(m.position)   # eaten: what is left lies there, no drops
	elif not m.is_baby():
		var list: Array = m.def.get("drops", []).duplicate()
		if not m.sheared:
			list.append_array(m.def.get("drops_unsheared", []))
		for s in MiningTable.roll_list(list, _rng):
			main.spawn_item(s, m.position + Vector3(0, 0.8, 0), Vector3(_rng.randf_range(-1, 1), 2.5, _rng.randf_range(-1, 1)))
	_poof(m.position + Vector3(0, 0.8, 0))
	stats["died"] += 1
	_active.erase(m)
	_hearts_t.erase(m.uid)
	m.queue_free()
	dirty = true


# ---------------- persistence ----------------

func to_dict() -> Dictionary:
	var mobs := []
	for m in _active:
		if is_instance_valid(m) and not m.is_dead():
			mobs.append(m.to_dict())
	for d in _dormant:
		mobs.append(d)
	var cells := []
	for c in _populated:
		cells.append([c.x, c.y])
	return {"version": SAVE_VERSION, "game_time": snappedf(game_time, 0.1), "next_uid": _next_uid, "cells": cells, "mobs": mobs}


func load_dict(d: Dictionary) -> bool:
	if int(d.get("version", 0)) != SAVE_VERSION:
		return false
	game_time = float(d.get("game_time", 0.0))
	_next_uid = int(d.get("next_uid", 1))
	_populated.clear()
	for c in d.get("cells", []):
		_populated[Vector2i(int(c[0]), int(c[1]))] = true
	_dormant = []
	for m in d.get("mobs", []):
		if m is Dictionary and defs.has(String(m.get("species", ""))):
			var md: Dictionary = m
			if not md.has("slept_at"):
				md["slept_at"] = game_time
			_dormant.append(md)   # they wake as soon as their ground is loaded
	dirty = false
	return true


func save_file(path: String) -> bool:
	DirAccess.make_dir_recursive_absolute(path.get_base_dir())
	var tmp := path + ".tmp"
	var f := FileAccess.open(tmp, FileAccess.WRITE)
	if f == null:
		return false
	f.store_string(JSON.stringify(to_dict()))
	f.close()
	if DirAccess.rename_absolute(tmp, path) != OK:
		return false
	dirty = false
	return true


func load_file(path: String) -> bool:
	if not FileAccess.file_exists(path):
		return false
	var data = JSON.parse_string(FileAccess.get_file_as_string(path))
	return data is Dictionary and load_dict(data)


func summary() -> Dictionary:
	var by := {}
	var goals := {}
	for m in _active:
		if not is_instance_valid(m):
			continue
		by[m.species] = int(by.get(m.species, 0)) + 1
		goals[m.goal] = int(goals.get(m.goal, 0)) + 1
	var sample := []
	for m in _active.slice(0, 6):
		if is_instance_valid(m):
			sample.append(m.summary())
	return {"active": _active.size(), "dormant": _dormant.size(), "frame_ms": snappedf(last_frame_ms, 0.001), "worst_frame_ms": snappedf(worst_frame_ms, 0.001), "avg_frame_ms": snappedf(_cost_sum / maxi(1, _cost_frames), 0.001),
		"step_ms": {"think": snappedf(part_us[0] / 1000.0 / maxi(1, _steps), 0.001), "move": snappedf(part_us[1] / 1000.0 / maxi(1, _steps), 0.001), "animate": snappedf(part_us[2] / 1000.0 / maxi(1, _steps), 0.001), "collide": snappedf(part_us[3] / 1000.0 / maxi(1, _steps), 0.001), "collides_per_step": snappedf(float(collides) / maxi(1, _steps), 0.1)},"by_species": by, "goals": goals, "stats": stats, "sample": sample}
