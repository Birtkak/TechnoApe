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

var world: World
var main: Node                 # get_world_voxel / set_world_voxel / spawn_item / player / player_inv
var defs := {}                 # species -> definition
var errors: Array[String] = []
var cell_v := 32
var chance := 0.1
var spawn_radius_v := 96.0
var active_radius_v := 128.0
var max_active := 60
var love_s := 30.0
var breed_cooldown_s := 300.0
var grow_up_s := 1200.0
var partner_radius_v := 16.0
var mate_s := 3.0
var feed_baby_speedup := 0.1
var spawning := true
var game_time := 0.0           # seconds this world has run (dormant mobs catch up from it)
var stats := {"spawned": 0, "herds": 0, "bred": 0, "died": 0, "slept": 0, "woke": 0, "cells": 0}
var dirty := false
var last_frame_ms := 0.0
var worst_frame_ms := 0.0
var _cost_sum := 0.0
var _cost_frames := 0
var frame_us := 0             # physics time of the manager and every mob since the last frame (Mob adds its own)

var _active: Array[Mob] = []
var _dormant: Array = []       # Mob.to_dict() + "slept_at"
var _populated := {}           # Vector2i cell -> true
var _next_uid := 1
var _scan_t := 0.0
var _hearts_t := {}            # uid -> seconds to the next heart
var _rng := RandomNumberGenerator.new()
var _seed := 0
var _searches := SEARCHES_PER_FRAME


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


## A heart, a puff, a pig and a chair in front of the camera while the loading screen is
## up: their first draw compiles pipelines and rasterizes the glyph (a 180 ms frame otherwise).
func _prewarm_fx() -> void:
	var cam: Camera3D = main.player.camera if main != null and main.player != null else null
	if cam == null:
		return
	var at := cam.global_position - cam.global_transform.basis.z * 3.0
	_heart(at)
	_poof(at)
	var warm := Node3D.new()
	warm.position = at
	add_child(warm)
	warm.add_child(MobModel.create("pig", false))
	warm.add_child(FurnitureEntity.new(Vector3i.ZERO, "chair", 0).make_view())
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
	game_time += delta
	_scan_t -= delta
	if _scan_t <= 0.0:
		_scan_t = 1.0
		_update_dormancy()
		if spawning:
			_update_spawning()
	frame_us += Time.get_ticks_usec() - t0


## Mob physics this frame into Main.phys_cost (DebugCapture lists it on a spike).
func _process(_delta: float) -> void:
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
		if not m.is_dead() and Vector2(m.position.x - pp.x, m.position.z - pp.z).length() > active_radius_v:
			var d := m.to_dict()
			d["slept_at"] = game_time
			_dormant.append(d)
			stats["slept"] += 1
			m.queue_free()
			continue
		keep.append(m)
	_active = keep
	if _dormant.is_empty():
		return
	var still := []
	for d in _dormant:
		var p: Array = d["pos"]
		var near := Vector2(float(p[0]) - pp.x, float(p[2]) - pp.z).length() < active_radius_v - 16.0
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
	if rng.randf() >= chance:
		return
	var cx := int((c.x + 0.5) * cell_v)
	var cz := int((c.y + 0.5) * cell_v)
	var biome := WorldGen.biome_name(int(world.generator.gen.column_info(cx, cz)["biome"]))
	var total := 0
	var options := []
	for k in defs:
		if biome in defs[k].get("biomes", []):
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
	var g: Array = defs[species].get("group", [2, 4])
	var n := rng.randi_range(int(g[0]), int(g[1]))
	var placed := 0
	var height := int(ceil(Settings.m2v(float(defs[species]["size_m"][1]))))
	for _i in n * 4:
		if placed >= n:
			break
		var x := cx + rng.randi_range(-10, 10)
		var z := cz + rng.randi_range(-10, 10)
		var h := world.surface_height(x, z)
		var y := stand_height(x, z, h + 1, 4, height)
		if y == -1 or get_voxel(Vector3i(x, y - 1, z)) != GameMaterials.GRASS:
			continue
		spawn(species, Vector3(x + 0.5, y + 0.05, z + 0.5), rng.randf() < 0.1, rng.randf() * TAU)
		placed += 1
	if placed > 0:
		stats["herds"] += 1


# ---------------- world queries (for Mob) ----------------

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
	var m := get_voxel(p)
	return m > 0 and _def_of(m).get("cell_type", "solid") == "liquid"


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
	var probe := m.position + dir * (m._half_v + 0.7)
	var x := int(floor(probe.x))
	var z := int(floor(probe.z))
	var y := int(floor(m.position.y + 0.05))
	for k in range(0, Mob.MAX_DROP_V + 2):
		var p := Vector3i(x, y - k, z)
		var v := get_voxel(p)
		if v == GameMaterials.LAVA:
			return false
		if v > 0 and _def_of(v).get("cell_type", "") == "liquid":
			return panicking or m.in_liquid
		if is_solid(v):
			return k <= Mob.MAX_DROP_V + 1
	return false


## Standing on grass or in a tuft: something to graze.
func can_graze(m: Mob) -> bool:
	var feet := Vector3i(m.position.floor())
	var here := get_voxel(feet)
	return here == GameMaterials.TALL_GRASS or get_voxel(feet + Vector3i.DOWN) == GameMaterials.GRASS


## Eats the tuft at the feet (it becomes air). Grass blocks are grazed but kept:
## nothing regrows grass yet (ADR 0022). Returns true when it ate.
func graze(m: Mob) -> bool:
	var feet := Vector3i(m.position.floor())
	if get_voxel(feet) == GameMaterials.TALL_GRASS:
		main.set_world_voxel(feet, GameMaterials.AIR)
		return true
	return get_voxel(feet + Vector3i.DOWN) == GameMaterials.GRASS


## A landing on farmland may trample it (Minecraft: babies are too light).
func landed(m: Mob, fall_m: float) -> void:
	if fall_m > 0.5 and not m.is_baby() and main != null and main.has_method("trample"):
		main.trample(Vector3i((m.position + Vector3(0, -0.1, 0)).floor()), fall_m)


## Pushes overlapping animals (and the player) apart, Minecraft's entity push.
func separate(m: Mob) -> void:
	for o in _active:
		if o == m or o.is_dead():
			continue
		var d := Vector2(m.position.x - o.position.x, m.position.z - o.position.z)
		var r := m._half_v + o._half_v
		if d.length() < r and absf(m.position.y - o.position.y) < 1.5:
			var push := d.normalized() if d.length() > 0.01 else Vector2(1, 0)
			m.velocity.x += push.x * 0.8
			m.velocity.z += push.y * 0.8
	var pp := _player_pos()
	var dp := Vector2(m.position.x - pp.x, m.position.z - pp.z)
	if dp.length() < m._half_v + Settings.m2v(Settings.player_radius_m) and absf(pp.y - m.position.y) < m._height_v:
		var push := dp.normalized() if dp.length() > 0.01 else Vector2(0, 1)
		m.velocity.x += push.x * 1.2
		m.velocity.z += push.y * 1.2


# ---------------- social ----------------

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


func _heart(at: Vector3) -> void:
	var l := Label3D.new()
	l.text = "♥"
	l.font_size = 64
	l.pixel_size = 0.012
	l.modulate = Color(1.0, 0.25, 0.35)
	l.outline_modulate = Color(0.35, 0.0, 0.05)
	l.outline_size = 8
	l.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	l.no_depth_test = false
	l.position = at
	add_child(l)
	var tw := l.create_tween()
	tw.set_parallel(true)
	tw.tween_property(l, "position", at + Vector3(0, 1.2, 0), 1.2)
	tw.tween_property(l, "modulate:a", 0.0, 1.2).set_delay(0.4)
	tw.chain().tween_callback(l.queue_free)


func _poof(at: Vector3) -> void:
	var p := CPUParticles3D.new()
	p.one_shot = true
	p.amount = 18
	p.lifetime = 0.8
	p.explosiveness = 0.9
	p.direction = Vector3.UP
	p.spread = 80.0
	p.initial_velocity_min = 1.0
	p.initial_velocity_max = 2.5
	p.gravity = Vector3(0, 0.6, 0)
	p.scale_amount_min = 0.6
	p.scale_amount_max = 1.2
	var mesh := BoxMesh.new()
	mesh.size = Vector3.ONE * 0.22
	var mat := StandardMaterial3D.new()
	mat.albedo_color = Color(0.92, 0.92, 0.9)
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mesh.material = mat
	p.mesh = mesh
	p.position = at
	add_child(p)
	p.emitting = true
	get_tree().create_timer(1.5).timeout.connect(p.queue_free)


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
	return m.hurt(damage, from)


## Right click on an animal with `held` (item def). Returns what happened:
## "love" / "grew" (fed: take one food), "sheared" (shears), or "".
func interact(m: Mob, held: Dictionary) -> String:
	if held.is_empty() or m.is_dead():
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
		for s in MiningTable.roll_list(m.def["shear"], _rng):
			main.spawn_item(s, m.eye(), Vector3(_rng.randf_range(-1.5, 1.5), 3.0, _rng.randf_range(-1.5, 1.5)))
		dirty = true
		return "sheared"
	return ""


func on_died(m: Mob) -> void:
	if not m.is_baby():
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
	return {"active": _active.size(), "dormant": _dormant.size(), "frame_ms": snappedf(last_frame_ms, 0.001), "worst_frame_ms": snappedf(worst_frame_ms, 0.001), "avg_frame_ms": snappedf(_cost_sum / maxi(1, _cost_frames), 0.001), "by_species": by, "goals": goals, "stats": stats, "sample": sample}
