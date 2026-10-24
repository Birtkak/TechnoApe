class_name Mob
extends Node3D

## One passive animal (ADR 0022): a VoxelBody walking the voxel grid (ADR 0011)
## and a goal brain modelled on Minecraft's animal goals, highest first:
##   panic      hurt: run to random spots nearby for 5 s, faster
##   breed      in love: walk to the nearest partner in love, mate for 3 s
##   tempt      a player within 10 m holds its food: follow, stop 2.5 m away,
##              watch them (5 s cooldown after the player puts it away)
##   follow     a baby keeps within 3 m of the nearest adult of its kind
##   graze      now and then (1/1000 per tick, babies 1/50) lower the head for
##              2 s and eat the grass tuft it stands on (regrows wool, ages babies)
##   stroll     every ~6 s walk to a random spot up to 10 m away (4 m up or down), preferring
##              grass and the herd, avoiding water, lava and drops over 3 m
##   look       idle: watch a player within 6 m or look around
## Predators (data "predator", ADR 0027: bears) think first about, in order:
##   eat        sit at a fresh kill for eat_s, head down
##   attack     aggressive ones (and any that were hurt by the player) charge a player in
##              survival within player_sight_m and bite every attack_s
##   hunt       when hungry (every hunt_every_s): pick the nearest prey within sight_m,
##              run it down (it bolts when the bear closes in), bite until it dies;
##              the kill leaves a carcass and the bear eats
## Chickens lay an egg every 5-10 minutes and flap down from heights without damage.
## Steering: walk straight at the target; the body steps up a 0.5 m block by
## itself and jumps a 1 m ledge (two blocks) when it bumps into one; when still
## blocked it slides along the wall for a second before giving up the target.
## Thinking runs at 10 Hz (staggered); movement every physics frame.
## World units are voxels; data (data/mobs.json) is in metres.

const THINK_S := 0.1
const TICK_S := 0.05            # Minecraft tick, for per-tick chances
const ACCEL := 24.0 * Settings.BLOCK   # voxels/s^2 toward the wanted velocity on the ground (12 m/s^2; distances here were tuned in 0.5 m blocks)
const AIR_ACCEL := 4.0 * Settings.BLOCK
const TURN := 6.0               # rad/s body turn rate
const HEAD_TURN := 8.0
const INVULNERABLE_S := 0.5     # Minecraft's 10 hurt ticks
const PANIC_S := 5.0
const GRAZE_S := 2.0
const DEATH_S := 1.0
const MAX_DROP_V := 6 * Settings.BLOCK   # never walk off more than 3 m
const BURN_S := 4.0             # alight this long after the last touch of lava or fire
const BURN_HURT_EVERY_S := 0.5

var species := ""
var def := {}
var manager: MobManager
var world: World
var uid := 0
var health := 8
var age_s := 0.0                # < 0: a baby, seconds until grown
var love_s := 0.0
var breed_cooldown_s := 0.0
var sheared := false
var goal := ""                  # what the brain is doing (JSON / F3)
var velocity := Vector3.ZERO
var on_floor := false
var in_liquid := false
var dying := 0.0                # 0 alive, then 0..1 over DEATH_S
var model: MobModel
var aggressive := false         # predators: also hunts the player (rolled once per animal, saved)
var killed_by := ""             # set when a predator's bite killed it: leaves a carcass instead of drops

var _body: VoxelBody
var _yaw := 0.0
var _head_yaw := 0.0
var _head_pitch := 0.0
var _think_t := 0.0
var _target := Vector3.INF      # where the legs go
var _speed := 0.0               # voxels/s toward _target
var _arrive := 0.8 * Settings.BLOCK   # voxels: close enough
var _look := Vector3.INF        # where the head looks (world point)
var _look_t := 0.0
var _panic_t := 0.0
var _graze_t := 0.0
var _mate_t := 0.0
var _tempt_cd := 0.0
var _was_tempted := false
var _hurt_t := 0.0
var _invuln_t := 0.0
var _slide_t := 0.0             # sliding along a wall: seconds left
var _slide_dir := Vector3.ZERO
var _stuck_t := 0.0
var _last_pos := Vector3.ZERO
var _fall_peak := 0.0
var _was_floor := true
var _rng := RandomNumberGenerator.new()
var _rest_t := 0.0
var _ahead_ok := true          # the brain's last cliff / water check along the walk direction
var _ahead_dir := Vector3.ZERO
var _height_v := 2.0
var _half_v := 0.9
var _prey: Mob = null
var _hunger_t := 0.0            # predators: seconds until it hunts again
var _attack_cd := 0.0
var _eat_t := 0.0
var _provoked_t := 0.0          # predators: hurt by the player, chases them for a while
var _lay_t := 0.0               # chickens: seconds to the next egg
var _burn_t := 0.0              # seconds still alight after touching lava or fire
var _burn_hurt_t := 0.0


static func create(p_manager: MobManager, p_species: String, p_def: Dictionary, baby: bool) -> Mob:
	var m := Mob.new()
	m.manager = p_manager
	m.world = p_manager.world
	m.species = p_species
	m.def = p_def
	m.health = int(p_def.get("health", 8))
	m.age_s = -p_manager.grow_up_s if baby else 0.0
	m.name = "Mob_" + p_species
	return m


func _ready() -> void:
	_rng.randomize()
	_think_t = _rng.randf() * THINK_S
	_yaw = rotation.y
	model = MobModel.create(species, sheared)
	add_child(model)
	_apply_size()
	_last_pos = position
	_fall_peak = position.y
	if def.has("predator"):
		var pr: Dictionary = def["predator"]
		var hv: Array = pr.get("hunt_every_s", [300, 600])
		_hunger_t = _rng.randf_range(float(hv[0]) * 0.3, float(hv[1]))
		if uid > 0 and not has_meta("rolled"):
			aggressive = float(hash([uid, species]) % 1000) / 1000.0 < float(pr.get("aggressive_chance", 0.0))
	if def.has("lays"):
		_lay_t = _lay_interval()


func is_baby() -> bool:
	return age_s < 0.0


func is_dead() -> bool:
	return dying > 0.0 or health <= 0


## Hitbox from data/mobs.json size_m [width, height] (halved for babies).
func _apply_size() -> void:
	var s: Array = def.get("size_m", [0.9, 0.9])
	var k := 0.5 if is_baby() else 1.0
	_half_v = Settings.m2v(float(s[0])) * 0.5 * k
	_height_v = Settings.m2v(float(s[1])) * k
	_body = VoxelBody.new(AABB(Vector3(-_half_v, 0, -_half_v), Vector3(_half_v * 2.0, _height_v, _half_v * 2.0)), Settings.m2v(0.55))
	if model:
		model.scale = Vector3.ONE * k * MobModel.UNIT
		model.head.scale = Vector3.ONE * (1.45 if is_baby() else 1.0)   # babies have big heads


func aabb() -> AABB:
	return AABB(position + Vector3(-_half_v, 0, -_half_v), Vector3(_half_v * 2.0, _height_v, _half_v * 2.0))


func eye() -> Vector3:
	return position + Vector3(0, _height_v * 0.8, 0)


# ---------------- interaction ----------------

## Damage from `from` (world position of the attacker). Returns true if it hurt.
## Lava or fire at the feet or in the body sets the animal alight, like the player's burning coat
## (PlayerStains): water puts it out (bug report 2026-10-25: a pig stood in lava unharmed).
func _touch_heat() -> void:
	if is_dead():
		return
	for f in [0.05, 0.4, 0.8]:
		var m := GameMaterials.material_of_voxel(manager.get_voxel(Vector3i((position + Vector3(0, _height_v * f, 0)).floor())))
		if m == GameMaterials.LAVA or m == GameMaterials.FIRE:
			if _burn_t <= 0.0:
				_burn_hurt_t = 0.0   # catching fire hurts at once
			_burn_t = BURN_S
			return
	if in_liquid:
		_burn_t = 0.0


func _burn_step(delta: float) -> void:
	if _burn_t <= 0.0:
		return
	_burn_t = maxf(0.0, _burn_t - delta)
	_burn_hurt_t -= delta
	if _burn_hurt_t <= 0.0:
		_burn_hurt_t = BURN_HURT_EVERY_S
		_invuln_t = 0.0   # the burn ticks on its own clock, not the hit cooldown
		hurt(1, position + Vector3(_rng.randf_range(-1, 1), 0, _rng.randf_range(-1, 1)))


func is_burning() -> bool:
	return _burn_t > 0.0


func hurt(amount: int, from: Vector3) -> bool:
	if is_dead() or _invuln_t > 0.0:
		return false
	health -= amount
	_invuln_t = INVULNERABLE_S
	_hurt_t = 0.35
	var away := position - from
	away.y = 0
	away = away.normalized() if away.length() > 0.01 else Vector3(_rng.randf_range(-1, 1), 0, _rng.randf_range(-1, 1)).normalized()
	var heavy := def.has("predator")
	velocity += away * Settings.m2v(1.5 if heavy else 4.0)
	velocity.y = Settings.m2v(1.5 if heavy else 3.0)
	if heavy:
		# predators turn on whoever hurt them instead of running (the player: chase for 30 s)
		var eye_p: Vector3 = manager.player_eye()
		if eye_p != Vector3.INF and from.distance_to(eye_p) < Settings.m2v(6.0):
			_provoked_t = 30.0
	else:
		_panic_t = PANIC_S
	_target = Vector3.INF
	_graze_t = 0.0
	if health <= 0:
		dying = 0.001
		goal = "dying"
	return true


## A predator closes in: bolt (prey only).
func scare() -> void:
	if not def.has("predator") and not is_dead():
		_panic_t = maxf(_panic_t, 3.0)


## Predators: the prey it was biting died at `at`: eat there, then rest until hungry again.
func on_kill(at: Vector3) -> void:
	var pr: Dictionary = def.get("predator", {})
	_prey = null
	_eat_t = float(pr.get("eat_s", 10.0))
	var hv: Array = pr.get("hunt_every_s", [300, 600])
	_hunger_t = _rng.randf_range(float(hv[0]), float(hv[1]))
	_look = at
	_look_t = _eat_t
	health = mini(int(def.get("health", health)), health + 4)


## /summon ... hungry: a predator hunts at once.
func make_hungry() -> void:
	_hunger_t = 0.0


func _lay_interval() -> float:
	var r: Array = def["lays"].get("every_s", [300, 600])
	return _rng.randf_range(float(r[0]), float(r[1]))


## The mob ate (feeding by hand): love, growth.
func feed() -> String:
	if is_baby():
		age_s = minf(0.0, age_s - age_s * manager.feed_baby_speedup)
		if age_s >= 0.0:
			grow_up()
		return "grew"
	if breed_cooldown_s <= 0.0 and love_s <= 0.0:
		love_s = manager.love_s
		return "love"
	return ""


func grow_up() -> void:
	age_s = 0.0
	_apply_size()


func set_sheared(on: bool) -> void:
	sheared = on
	if model:
		model.set_sheared(on)


# ---------------- per frame ----------------

func _physics_process(delta: float) -> void:
	var t0 := Time.get_ticks_usec()
	_step(delta)
	manager.frame_us += Time.get_ticks_usec() - t0


func _step(delta: float) -> void:
	if dying > 0.0:
		dying += delta / DEATH_S
		model.animate(delta, 0.0, 0.0, 0.0, 0.0, minf(dying, 1.0))
		model.set_hurt(1.0)
		if dying >= 1.0:
			manager.on_died(self)
		return
	if not world.is_area_loaded(Vector3i((position / Settings.CHUNK).floor())):
		return   # never fall into terrain that has not streamed in
	_timers(delta)
	_think_t -= delta
	if _think_t <= 0.0:
		_think_t += THINK_S
		_think()
	_move(delta)
	_animate(delta)


func _timers(delta: float) -> void:
	_invuln_t = maxf(0.0, _invuln_t - delta)
	_panic_t = maxf(0.0, _panic_t - delta)
	_tempt_cd = maxf(0.0, _tempt_cd - delta)
	_look_t = maxf(0.0, _look_t - delta)
	_slide_t = maxf(0.0, _slide_t - delta)
	if _hurt_t > 0.0:
		_hurt_t = maxf(0.0, _hurt_t - delta)
		model.set_hurt(_hurt_t / 0.35)
	if love_s > 0.0:
		love_s = maxf(0.0, love_s - delta)
		manager.hearts_tick(self, delta)
	if breed_cooldown_s > 0.0:
		breed_cooldown_s = maxf(0.0, breed_cooldown_s - delta)
	if age_s < 0.0:
		age_s += delta
		if age_s >= 0.0:
			grow_up()
	if _graze_t > 0.0:
		_graze_t -= delta
		if _graze_t <= 0.0:
			_finish_graze()
	_attack_cd = maxf(0.0, _attack_cd - delta)
	_provoked_t = maxf(0.0, _provoked_t - delta)
	_burn_step(delta)
	if _eat_t > 0.0:
		_eat_t = maxf(0.0, _eat_t - delta)
	elif _hunger_t > 0.0:
		_hunger_t -= delta
	if _lay_t > 0.0 and not is_baby():
		_lay_t -= delta
		if _lay_t <= 0.0:
			_lay_t = _lay_interval()
			manager.lay(self)


# ---------------- brain ----------------

func _chance_per_tick(p: float) -> bool:
	return _rng.randf() < 1.0 - pow(1.0 - p, THINK_S / TICK_S)


func _think() -> void:
	manager.separate(self)
	in_liquid = manager.is_liquid(Vector3i((position + Vector3(0, _height_v * 0.4, 0)).floor()))
	_touch_heat()
	_think_goal()
	# voxel probes run here at 10 Hz, never per frame (they are GDScript voxel reads)
	if _target != Vector3.INF and _speed > 0.0:
		var to := _target - position
		to.y = 0
		_ahead_dir = _slide_dir if _slide_t > 0.0 else to.normalized()
		_ahead_ok = manager.safe_ahead(self, _ahead_dir, _panic_t > 0.0)
	else:
		_ahead_ok = true


func _think_goal() -> void:
	if def.has("predator") and _predator_goal():
		return
	# panic: run to random spots around until it wears off
	if _panic_t > 0.0:
		goal = "panic"
		if _target == Vector3.INF or _reached():
			_go(_random_land_target(10 * Settings.BLOCK, 8 * Settings.BLOCK, false), Settings.m2v(float(def.get("panic_m", 2.5))))
		return
	# breed
	if love_s > 0.0 and not is_baby():
		var mate := manager.find_partner(self)
		if mate != null:
			goal = "breed"
			_look = mate.eye()
			_look_t = 0.5
			if position.distance_to(mate.position) < 4.0 * Settings.BLOCK:
				_stop()
				_mate_t += THINK_S
				if _mate_t >= manager.mate_s and uid < mate.uid:
					manager.breed(self, mate)
			else:
				_mate_t = 0.0
				_go(mate.position, Settings.m2v(float(def.get("walk_m", 1.0))), 3.0 * Settings.BLOCK)
			return
	_mate_t = 0.0
	# tempt: follow a player holding our food
	var lure: Vector3 = manager.tempt_target(self) if _tempt_cd <= 0.0 else Vector3.INF
	if lure != Vector3.INF:
		goal = "tempt"
		_was_tempted = true
		_graze_t = 0.0
		_look = lure + Vector3(0, Settings.m2v(1.5), 0)
		_look_t = 0.5
		if Vector2(lure.x - position.x, lure.z - position.z).length() < Settings.m2v(2.5):
			_stop()
		else:
			_go(lure, Settings.m2v(float(def.get("tempt_m", 1.3))), Settings.m2v(2.5))
		return
	if _was_tempted:
		_was_tempted = false
		_tempt_cd = 5.0   # Minecraft: 100 ticks before it can be tempted again
		_stop()
	# babies follow a grown-up
	if is_baby():
		var parent := manager.nearest_adult(self)
		if parent != null and position.distance_to(parent.position) > Settings.m2v(3.0):
			goal = "follow"
			_go(parent.position, Settings.m2v(float(def.get("walk_m", 1.0)) * 1.1), Settings.m2v(2.0))
			return
	# graze
	if _graze_t > 0.0:
		goal = "graze"
		return
	if def.get("grazes", false) and on_floor and _chance_per_tick(0.02 if is_baby() else 0.001) and manager.can_graze(self):
		goal = "graze"
		_graze_t = GRAZE_S
		_stop()
		return
	# stroll
	if _target != Vector3.INF and not _reached():
		goal = "stroll"
		return
	if _chance_per_tick(1.0 / 120.0):
		var t := _random_land_target(20 * Settings.BLOCK, 8 * Settings.BLOCK, true)
		if t != Vector3.INF:
			goal = "stroll"
			_go(t, Settings.m2v(float(def.get("walk_m", 1.0))))
			return
	goal = "idle"
	_stop()
	# look at a player nearby or around
	if _look_t <= 0.0:
		var p: Vector3 = manager.player_eye()
		if p.distance_to(eye()) < Settings.m2v(6.0) and _chance_per_tick(0.02):
			_look = p
			_look_t = _rng.randf_range(2.0, 4.0)
		elif _chance_per_tick(0.02):
			var a := _yaw + _rng.randf_range(-1.6, 1.6)
			_look = eye() + Vector3(sin(a), _rng.randf_range(-0.3, 0.2), cos(a)) * 6.0 * Settings.BLOCK
			_look_t = _rng.randf_range(1.0, 2.0)


## Predator goals (eat, attack the player, hunt); false = fall through to the animal goals.
func _predator_goal() -> bool:
	var pr: Dictionary = def["predator"]
	var run := Settings.m2v(float(pr.get("chase_m", 4.0)))
	var reach := Settings.m2v(float(pr.get("reach_m", 1.6))) + _half_v
	if _eat_t > 0.0:
		goal = "eat"
		_stop()
		return true
	if (aggressive or _provoked_t > 0.0) and manager.player_attackable():
		var pp: Vector3 = manager.player_feet()
		var d := pp.distance_to(position)
		var sight := Settings.m2v(float(pr.get("player_sight_m", 14.0)))
		if d < sight or (_provoked_t > 0.0 and d < sight * 2.5):
			goal = "attack"
			_look = manager.player_eye()
			_look_t = 0.5
			if d < reach + Settings.m2v(0.3):
				_stop()
				if _attack_cd <= 0.0:
					_attack_cd = float(pr.get("attack_s", 1.2))
					manager.maul_player(self, int(pr.get("attack", 6)))
			else:
				_go(pp, run, 0.5 * Settings.BLOCK)
			return true
	if _hunger_t <= 0.0 and not is_baby():
		if _prey == null or not is_instance_valid(_prey) or _prey.is_dead():
			_prey = manager.find_prey(self)
			if _prey == null:
				_hunger_t = 20.0   # nothing about: look again later
				return false
		var d := _prey.position.distance_to(position)
		if d > Settings.m2v(float(pr.get("sight_m", 24.0))) * 1.5:
			_prey = null
			_hunger_t = 30.0   # it got away
			return false
		goal = "hunt"
		_look = _prey.eye()
		_look_t = 0.5
		if d < Settings.m2v(8.0):
			_prey.scare()
		if d < reach + _prey._half_v:
			_stop()
			if _attack_cd <= 0.0:
				_attack_cd = float(pr.get("attack_s", 1.2))
				manager.maul(self, _prey, int(pr.get("attack", 6)))
		else:
			_go(_prey.position, run, 0.5 * Settings.BLOCK)
		return true
	return false


func _go(t: Vector3, speed: float, arrive: float = 0.8 * Settings.BLOCK) -> void:
	if t == Vector3.INF:
		return
	if _target == Vector3.INF or _target.distance_to(t) > 0.5 * Settings.BLOCK:
		_stuck_t = 0.0
	_target = t
	_speed = speed
	_arrive = arrive


func _stop() -> void:
	_target = Vector3.INF
	_speed = 0.0


func _reached() -> bool:
	return Vector2(_target.x - position.x, _target.z - position.z).length() <= _arrive


## Minecraft's LandRandomPos: up to 8 tries (Minecraft: 10) for a standable spot within h voxels
## horizontally and v vertically; animals score grass highest; stroll also likes
## staying near the herd.
func _random_land_target(h: int, v: int, herd: bool) -> Vector3:
	var best := Vector3.INF
	if not manager.take_search():
		return best   # over this frame's budget: the brain tries again next think
	var best_score := -1e9
	var centre: Vector3 = manager.herd_centre(self) if herd else Vector3.INF
	for _i in 8:
		var x := int(floor(position.x)) + _rng.randi_range(-h, h)
		var z := int(floor(position.z)) + _rng.randi_range(-h, h)
		var y := manager.stand_height(x, z, int(floor(position.y)), v, int(ceil(_height_v)))
		if y == -1:
			continue
		var p := Vector3(x + 0.5, y, z + 0.5)
		var score := _rng.randf()
		if manager.get_voxel(Vector3i(x, y - 1, z)) == GameMaterials.GRASS:
			score += 10.0
		if centre != Vector3.INF:
			score -= maxf(0.0, (p.distance_to(centre) - 8.0 * Settings.BLOCK) / Settings.BLOCK) * 0.6
		if score > best_score:
			best_score = score
			best = p
	return best


func _finish_graze() -> void:
	if manager.graze(self):
		if sheared:
			set_sheared(false)
		if is_baby():
			age_s = minf(0.0, age_s + 60.0)   # Minecraft: eating grass ages a lamb by a minute
			if age_s >= 0.0:
				grow_up()


# ---------------- movement ----------------

func _move(delta: float) -> void:
	var want := Vector3.ZERO
	if _target != Vector3.INF and _speed > 0.0:
		if _reached():
			_stop()
		else:
			var to := _target - position
			to.y = 0
			var dir := to.normalized()
			if _slide_t > 0.0:
				dir = _slide_dir
			elif not _ahead_ok and dir.dot(_ahead_dir) > 0.7:
				# a cliff, water or lava ahead: give the target up (panic picks another)
				_stop()
				_ahead_ok = true
				dir = Vector3.ZERO
			want = dir * _speed * (0.5 if in_liquid else 1.0)
	# at rest on the ground: skip the collision move, re-check the floor 4 times a second
	# (like ItemEntity's sleep; most animals stand still most of the time)
	if want == Vector3.ZERO and on_floor and not in_liquid and Vector3(velocity.x, 0, velocity.z).length() < 0.05 * Settings.BLOCK:
		_rest_t -= delta
		if _rest_t > 0.0:
			return
		_rest_t = 0.25
		velocity = Vector3(0, -1.0, 0)
	var acc := (ACCEL if on_floor else AIR_ACCEL) * delta
	velocity.x = move_toward(velocity.x, want.x, acc)
	velocity.z = move_toward(velocity.z, want.z, acc)
	if in_liquid:
		# Minecraft's FloatGoal: bob up to the surface
		velocity.y = move_toward(velocity.y, Settings.m2v(1.6), Settings.m2v(Settings.gravity_m) * 0.6 * delta)
	else:
		velocity.y -= Settings.m2v(Settings.gravity_m) * delta
		if def.get("slow_fall", false):
			velocity.y = maxf(velocity.y, -Settings.m2v(1.2))   # chickens flap down
	var got := _body.move(position, velocity, delta, world.terrain, on_floor)
	position += got
	var blocked := _body.blocked.x > 0.0 or _body.blocked.z > 0.0
	on_floor = _body.on_floor
	if _body.blocked.x > 0.0:
		velocity.x = 0.0
	if _body.blocked.z > 0.0:
		velocity.z = 0.0
	if on_floor or _body.hit_ceiling:
		velocity.y = 0.0
	if blocked and want != Vector3.ZERO:
		if on_floor and not in_liquid:
			velocity.y = sqrt(2.0 * Settings.m2v(Settings.gravity_m) * Settings.m2v(1.15))   # hop a 1 m ledge (two blocks)
		_stuck_t += delta
		if _stuck_t > 0.6 and _slide_t <= 0.0:
			# slide along the wall to one side for a moment
			var side := 1.0 if _rng.randf() < 0.5 else -1.0
			_slide_dir = want.normalized().rotated(Vector3.UP, side * PI * 0.5)
			_slide_t = 1.0
		if _stuck_t > 2.5:
			_stop()
			_stuck_t = 0.0
	elif want != Vector3.ZERO and _slide_t <= 0.0:
		_stuck_t = maxf(0.0, _stuck_t - delta)
	_land(delta)
	# face where it walks
	var hv := Vector2(velocity.x, velocity.z)
	if want != Vector3.ZERO and hv.length() > 0.2 * Settings.BLOCK:
		_yaw = rotate_toward(_yaw, atan2(velocity.x, velocity.z), TURN * delta)
	rotation.y = _yaw


## Fall damage (Minecraft: 1 per metre beyond 3) and trampling farmland.
func _land(_delta: float) -> void:
	if on_floor and not _was_floor:
		var fall_m := Settings.v2m(_fall_peak - position.y)
		if not in_liquid:
			manager.landed(self, fall_m)
			var dmg := 0 if def.get("slow_fall", false) else int(ceil(fall_m - 3.0))
			if dmg > 0:
				hurt(dmg, position)
	if on_floor or in_liquid or position.y > _fall_peak:
		_fall_peak = position.y
	_was_floor = on_floor or in_liquid


func _animate(delta: float) -> void:
	var yaw_want := 0.0
	var pitch_want := 0.0
	if _look_t > 0.0 and _look != Vector3.INF:
		var d := _look - eye()
		yaw_want = clampf(wrapf(atan2(d.x, d.z) - _yaw, -PI, PI), -1.0, 1.0)
		pitch_want = clampf(-atan2(d.y, Vector2(d.x, d.z).length()), -0.6, 0.6)
	_head_yaw = move_toward(_head_yaw, yaw_want, HEAD_TURN * delta)
	_head_pitch = move_toward(_head_pitch, pitch_want, HEAD_TURN * delta)
	var graze := clampf(minf(_graze_t, GRAZE_S - _graze_t) * 4.0, 0.0, 1.0) if _graze_t > 0.0 else 0.0
	if _eat_t > 0.0:
		graze = clampf(_eat_t * 2.0, 0.0, 1.0)   # tearing at the kill
	model.flapping = not on_floor and not in_liquid
	model.animate(delta, Vector2(velocity.x, velocity.z).length(), graze, _head_yaw, _head_pitch, 0.0)


# ---------------- saving ----------------

func to_dict() -> Dictionary:
	return {"species": species, "uid": uid, "pos": [snappedf(position.x, 0.01), snappedf(position.y, 0.01), snappedf(position.z, 0.01)],
		"yaw": snappedf(_yaw, 0.01), "health": health, "age_s": snappedf(age_s, 0.1), "love_s": snappedf(love_s, 0.1),
		"breed_cooldown_s": snappedf(breed_cooldown_s, 0.1), "sheared": sheared, "aggressive": aggressive}


func load_from(d: Dictionary) -> void:
	var p: Array = d.get("pos", [0, 0, 0])
	position = Vector3(float(p[0]), float(p[1]), float(p[2]))
	rotation.y = float(d.get("yaw", 0.0))
	uid = int(d.get("uid", 0))
	health = int(d.get("health", health))
	age_s = float(d.get("age_s", 0.0))
	love_s = float(d.get("love_s", 0.0))
	breed_cooldown_s = float(d.get("breed_cooldown_s", 0.0))
	sheared = bool(d.get("sheared", false))
	if d.has("aggressive"):
		aggressive = bool(d["aggressive"])
		set_meta("rolled", true)


func summary() -> Dictionary:
	return {"species": species, "uid": uid, "goal": goal, "pos": position.snapped(Vector3.ONE * 0.1), "health": health,
		"baby": is_baby(), "love": love_s > 0.0, "sheared": sheared, "aggressive": aggressive, "burning": is_burning()}
