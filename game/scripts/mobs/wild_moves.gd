class_name WildMoves
extends RefCounted

## What the wild animals of ADR 0058 do beyond Mob's Minecraft goals, data in data/mobs.json:
##   skittish_m  flee a player who comes this close (deer, rabbits, foxes, ducks, frogs, crabs, fish)
##   moves       hop: short hops (rabbits, frogs); swim: fish move in 3D through water and flop and
##               suffocate on land; fly: bats flutter in 3D through cave air; paddle: ducks walk and
##               sometimes paddle out onto water
##   habitat     land (Mob's herds), shore and water (rolled per cell where water is near), cave (rolled
##               per cell, in cave air below the surface)
##   predator.pack  wolves join a pack mate's hunt
## Mob and MobManager call in through small hooks; per-animal state lives in metadata so Mob keeps
## its fields.

const FLOP_HURT_S := 2.0       # a fish on land loses a half-heart this often
const CAVE_CHANCE := 0.12      # per spawn cell: a cave group below it
const WATER_CHANCE := 0.25     # per spawn cell with water near its centre: fish or shore animals
const CAVE_SCAN_M := 40.0      # how far below the surface a cave group may spawn


static func moves(m: Mob) -> String:
	return String(m.def.get("moves", "walk"))


static func habitat(def: Dictionary) -> String:
	return String(def.get("habitat", "land"))


## Ground a land herd may spawn on.
static func ground_ok(def: Dictionary, ground: int) -> bool:
	if def.has("ground"):
		for g in def["ground"]:
			if ground == GameMaterials.id_of(String(g)):
				return true
		return false
	return GameMaterials.is_grass(ground) or ground == GameMaterials.SNOW


# ---------------- brain ----------------

## Mob._think_goal, after the predator goals: true when this took the turn.
static func think(m: Mob) -> bool:
	var mv := moves(m)
	var sk := float(m.def.get("skittish_m", 0.0))
	if sk > 0.0 and not m.is_dead():
		var pp: Vector3 = m.manager.player_feet()
		var away := m.position - pp
		away.y = 0.0
		if pp != Vector3.ZERO and away.length() < Settings.m2v(sk) and _player_visible(m):
			m.goal = "flee"
			m._panic_t = maxf(m._panic_t, 1.5)
			if m._target == Vector3.INF or m._reached() or m.position.distance_to(m._target) < Settings.m2v(2.0):
				var dir := away.normalized() if away.length() > 0.01 else Vector3(1, 0, 0)
				dir = dir.rotated(Vector3.UP, m._rng.randf_range(-0.6, 0.6))
				var t := m.position + dir * Settings.m2v(8.0)
				if mv == "swim" or mv == "fly":
					m._go(t, Settings.m2v(float(m.def.get("panic_m", 2.5))))
				else:
					var y := m.manager.stand_height(int(floor(t.x)), int(floor(t.z)), int(floor(m.position.y)), 4 * Settings.BLOCK, int(ceil(m._height_v)))
					if y != -1:
						m._go(Vector3(floor(t.x) + 0.5, y, floor(t.z) + 0.5), Settings.m2v(float(m.def.get("panic_m", 2.5))))
			return true
	if mv == "swim" or mv == "fly":
		m.goal = "panic" if m._panic_t > 0.0 else ("swim" if mv == "swim" else "fly")
		if m._target == Vector3.INF or m.position.distance_to(m._target) < Settings.m2v(0.6) or m._chance_per_tick(1.0 / 160.0):
			var t := _target_3d(m, mv == "swim")
			if t != Vector3.INF:
				var sp := float(m.def.get("panic_m" if m._panic_t > 0.0 else "walk_m", 1.0))
				m._go(t, Settings.m2v(sp), Settings.m2v(0.5))
		return true
	if mv == "paddle" and m._target == Vector3.INF and m._panic_t <= 0.0 and m._chance_per_tick(1.0 / 300.0):
		var t := _water_surface_near(m, 10)
		if t != Vector3.INF:
			m.goal = "paddle"
			m._go(t, Settings.m2v(float(m.def.get("walk_m", 1.0))))
			return true
	return false


static func _player_visible(m: Mob) -> bool:
	var main: Node = m.manager.main
	if main == null or main.player == null:
		return false
	return String(main.player.get("game_mode")) != "spectator"


## A predator's prey: a pack mate's prey first (wolves), else its own pick.
static func pack_prey(m: Mob, own: Mob) -> Mob:
	if not m.def.get("predator", {}).get("pack", false):
		return own
	for o in m.manager.active():
		if o != m and o.species == m.species and is_instance_valid(o._prey) and not o._prey.is_dead() \
				and o.position.distance_to(m.position) < Settings.m2v(24.0):
			return o._prey
	return own


## A random spot in water (fish) or cave air (bats) within a few metres.
static func _target_3d(m: Mob, water: bool) -> Vector3:
	if not m.manager.take_search():
		return Vector3.INF
	var h := Settings.m2v(4.0)
	for _i in 6:
		var t := m.position + Vector3(m._rng.randf_range(-h, h), m._rng.randf_range(-h, h) * 0.4, m._rng.randf_range(-h, h))
		var p := Vector3i(t.floor())
		if water:
			if m.manager.is_liquid(p) and m.manager.is_liquid(p + Vector3i.UP):
				return t
		elif m.manager.get_voxel(p) == GameMaterials.AIR and m.manager.get_voxel(Vector3i(((m.position + t) * 0.5).floor())) == GameMaterials.AIR:
			return t
	return Vector3.INF


## The top of open water within r voxels of a duck, or INF.
static func _water_surface_near(m: Mob, r_m: int) -> Vector3:
	if not m.manager.take_search():
		return Vector3.INF
	var r := Settings.m2v(r_m)
	for _i in 6:
		var x := int(floor(m.position.x + m._rng.randf_range(-r, r)))
		var z := int(floor(m.position.z + m._rng.randf_range(-r, r)))
		var y0 := int(floor(m.position.y))
		for y in range(y0 + 4, y0 - 12, -1):
			var p := Vector3i(x, y, z)
			if m.manager.is_liquid(p) and m.manager.get_voxel(p + Vector3i.UP) == GameMaterials.AIR:
				return Vector3(x + 0.5, y + 0.6, z + 0.5)
			if MobManager.is_solid(m.manager.get_voxel(p)):
				break
	return Vector3.INF


# ---------------- movement ----------------

## Mob._move, first: swimmers and fliers move here (true = done for this frame).
static func move(m: Mob, delta: float) -> bool:
	var mv := moves(m)
	if mv != "swim" and mv != "fly":
		return false
	var want := Vector3.ZERO
	if m._target != Vector3.INF and m._speed > 0.0:
		var to := m._target - m.position
		if to.length() <= m._arrive * 0.5:
			m._stop()
		else:
			want = to.normalized() * m._speed
	var acc := Mob.ACCEL * 0.35 * delta
	if mv == "swim" and not m.in_liquid:
		# out of the water: flop, and suffocate
		m.velocity.x = move_toward(m.velocity.x, 0.0, acc)
		m.velocity.z = move_toward(m.velocity.z, 0.0, acc)
		m.velocity.y -= Settings.m2v(Settings.gravity_m) * delta
		if m.on_floor and m._rng.randf() < 3.0 * delta:
			m.velocity += Vector3(m._rng.randf_range(-1, 1), 1.6, m._rng.randf_range(-1, 1)) * Settings.m2v(1.2)
		var t: float = m.get_meta("flop_t", FLOP_HURT_S) - delta
		if t <= 0.0:
			t = FLOP_HURT_S
			m._invuln_t = 0.0
			m.hurt(1, m.position)
		m.set_meta("flop_t", t)
	else:
		if mv == "fly":
			want.y += sin(m.model._time * 7.0) * Settings.m2v(0.6)   # the flutter
		m.velocity = m.velocity.move_toward(want, acc)
	var got := m._body.move(m.position, m.velocity, delta, m.world.terrain, m.on_floor)
	m.position += got
	m.on_floor = m._body.on_floor
	if m._body.blocked.x > 0.0 or m._body.blocked.z > 0.0 or m._body.hit_ceiling or (m.on_floor and mv == "fly"):
		m._target = Vector3.INF   # bumped: pick another spot
		if m._body.blocked.x > 0.0:
			m.velocity.x = 0.0
		if m._body.blocked.z > 0.0:
			m.velocity.z = 0.0
		if m._body.hit_ceiling or m.on_floor:
			m.velocity.y = 0.0
	var hv := Vector2(m.velocity.x, m.velocity.z)
	if hv.length() > 0.1 * Settings.BLOCK:
		m._yaw = rotate_toward(m._yaw, atan2(m.velocity.x, m.velocity.z), Mob.TURN * delta)
	m.rotation.y = m._yaw
	return true


## Mob._move, before the body moves: hoppers jump along instead of walking.
static func hop(m: Mob, want: Vector3) -> void:
	if moves(m) != "hop" or not m.on_floor or want == Vector3.ZERO or m.in_liquid:
		return
	var h := 0.35 if m.species != "frog" else 0.25
	m.velocity.y = sqrt(2.0 * Settings.m2v(Settings.gravity_m) * Settings.m2v(h))


## MobManager.safe_ahead: swimmers and paddlers may go into water.
static func water_ok(m: Mob) -> bool:
	var mv := moves(m)
	return mv == "paddle" or mv == "swim"


# ---------------- spawning ----------------

## MobManager._populate: shore, water and cave groups get their own rolls per spawn cell.
static func populate(mm: MobManager, c: Vector2i) -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = hash([mm._seed, c.x, c.y, 0xE5A7])
	var cx := int((c.x + 0.5) * mm.cell_v)
	var cz := int((c.y + 0.5) * mm.cell_v)
	if rng.randf() < WATER_CHANCE:
		_populate_water(mm, rng, cx, cz)
	if rng.randf() < CAVE_CHANCE:
		_populate_cave(mm, rng, cx, cz)


static func _pick(mm: MobManager, rng: RandomNumberGenerator, habitats: Array, biome: String) -> String:
	var options := []
	var total := 0
	for k in mm.defs:
		var d: Dictionary = mm.defs[k]
		if habitat(d) in habitats and (biome in d.get("biomes", []) or "cave" in habitats):
			options.append(k)
			total += int(d.get("weight", 1))
	if total <= 0:
		return ""
	var pick := rng.randi_range(0, total - 1)
	for k in options:
		pick -= int(mm.defs[k].get("weight", 1))
		if pick < 0:
			return k
	return options[0]


## Fish in water near the cell centre, ducks and frogs on its banks.
static func _populate_water(mm: MobManager, rng: RandomNumberGenerator, cx: int, cz: int) -> void:
	var biome := WorldGen.biome_name(int(mm.world.generator.gen.column_info(cx, cz)["biome"]))
	for _i in 6:
		var x := cx + rng.randi_range(-12, 12) * Settings.BLOCK
		var z := cz + rng.randi_range(-12, 12) * Settings.BLOCK
		var h := mm.world.surface_height(x, z)
		var top := -1
		for y in range(h + 2, h - 2 * Settings.BLOCK, -1):
			if mm.is_liquid(Vector3i(x, y, z)):
				top = y
				break
		if top == -1:
			continue
		var depth := 0
		while depth < 3 * Settings.BLOCK and mm.is_liquid(Vector3i(x, top - depth - 1, z)):
			depth += 1
		var sp := _pick(mm, rng, ["water"] if depth >= 4 else ["shore"], biome)
		if sp == "":
			return
		var g: Array = mm.defs[sp].get("group", [2, 4])
		var n := rng.randi_range(int(g[0]), int(g[1]))
		var placed := 0
		for _k in n * 4:
			if placed >= n:
				break
			var px := x + rng.randi_range(-4, 4) * Settings.BLOCK
			var pz := z + rng.randi_range(-4, 4) * Settings.BLOCK
			var pos := Vector3.INF
			if habitat(mm.defs[sp]) == "water":
				var p := Vector3i(px, top - 2, pz)
				if mm.is_liquid(p) and mm.is_liquid(p + Vector3i.UP):
					pos = Vector3(px + 0.5, top - 2, pz + 0.5)
			else:
				var y := mm.stand_height(px, pz, top + 1, 3 * Settings.BLOCK, int(ceil(Settings.m2v(float(mm.defs[sp]["size_m"][1])))))
				if y != -1:
					pos = Vector3(px + 0.5, y + 0.05, pz + 0.5)
			if pos != Vector3.INF:
				mm.spawn(sp, pos, rng.randf() < 0.1, rng.randf() * TAU)
				placed += 1
		if placed > 0:
			mm.stats["herds"] += 1
		return


## Bats and cave spiders in a cave pocket below the cell centre.
static func _populate_cave(mm: MobManager, rng: RandomNumberGenerator, cx: int, cz: int) -> void:
	var sp := _pick(mm, rng, ["cave"], "")
	if sp == "":
		return
	var d: Dictionary = mm.defs[sp]
	var height := int(ceil(Settings.m2v(float(d["size_m"][1]))))
	for _i in 3:
		var x := cx + rng.randi_range(-8, 8) * Settings.BLOCK
		var z := cz + rng.randi_range(-8, 8) * Settings.BLOCK
		var h := mm.world.surface_height(x, z)
		var lo := h - int(Settings.m2v(CAVE_SCAN_M))
		if not mm.world.is_area_loaded(Vector3i(floori(float(x) / Settings.CHUNK), floori(float(lo) / Settings.CHUNK), floori(float(z) / Settings.CHUNK))):
			continue
		var y := mm.stand_height(x, z, (h + lo) / 2, (h - lo) / 2 - Settings.m2v(5.0), height + Settings.BLOCK)
		if y == -1 or y > h - Settings.m2v(6.0):
			continue
		var g: Array = d.get("group", [1, 3])
		var n := rng.randi_range(int(g[0]), int(g[1]))
		for k in n:
			var up := Settings.m2v(1.5) if moves_of(d) == "fly" else 0.05
			mm.spawn(sp, Vector3(x + 0.5 + k * 0.6, y + up, z + 0.5), false, rng.randf() * TAU)
		mm.stats["herds"] += 1
		return


static func moves_of(def: Dictionary) -> String:
	return String(def.get("moves", "walk"))
