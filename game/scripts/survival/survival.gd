class_name Survival
extends Node

## Hunger, body temperature and food spoilage (ADR 0040). The rules are static functions (tested
## headless); this node gathers what they need (climate, calendar, shelter and heat from the native
## ClimateProbe, lit entities), keeps the state, ages perishable stacks, and hurts or slows the player.
## Creative mode keeps the readings and never hurts or slows. State saves with the player.
## Food groups (ADR 0061, `Nutrition`) drain beside hunger and set max health; containers in a root
## cellar (a closed room under ground) and "preserved" foods spoil slower (`Seasons`).

const DAY_S := 1200.0       # one game day of real seconds (the calendar's 20-minute day)
const HEAT_C := 1.0         # degrees per unit of heat (a lit firepit is 10 units at its edge)
## Heat of lit entities by kind, in the probe's units, and of hot cells (per cell).
const ENTITY_HEAT := {"firepit": 10.0, "forge": 12.0, "bloomery": 10.0, "furnace": 8.0, "firewood_pile": 14.0, "ground": 8.0, "torch": 1.5, "boiler": 12.0, "blast_furnace": 14.0}
const ENTITY_HEAT_M := 8.0  # lit entities further than this give nothing

var main: Node
var player: Player
var probe := ClimateProbe.new()
var clothing: Clothing = null   # what the player wears (ADR 0059), set by StoneAge

var hunger := 20.0
var saturation := 5.0
var body_c := 37.0
var env_c := 18.0
var outside_c := 18.0   # the open-air temperature here: no roof, no heat, not in water
var in_room := false
var heat := 0.0
var underground_m := 0.0
var _probe_t := 0.0
var _starve_t := 0.0
var _regen_t := 0.0
var _hurt_t := 0.0
var _spoil_t := 0.0
var _last_days := -1.0
var _open := PackedByteArray()
var _heat_mat := PackedFloat32Array()
var nutrition := Nutrition.load_default()
var _health_t := 0.0
var _rooms := {}             # container position -> [closed room, valid until clock s] (root cellars)
const ROOM_CACHE_S := 30.0
var stats := {"probe_ms_max": 0.0, "spoil_ms_max": 0.0, "rotted": 0, "eaten": 0, "starved": 0, "froze": 0, "overheated": 0}


func setup(p_main: Node) -> void:
	main = p_main
	player = main.player
	player.survival = self
	_open.resize(256)
	_heat_mat.resize(256)
	for i in GameMaterials.defs().size():
		var d: Dictionary = GameMaterials.defs()[i]
		var m := i + 1
		var ct: String = d.get("cell_type", "solid")
		# the fill passes air, gases, fire, liquids, plants and leaves (a canopy is not a roof)
		_open[m] = 1 if ct in ["gas", "fire", "liquid"] or d.get("replaceable", false) or d.has("micro") and not GameMaterials.SOLID_MICRO.has(d["micro"]) or d.get("rigid", "") == "leaf" else 0
	_open[GameMaterials.AIR] = 1
	_heat_mat[GameMaterials.FIRE] = 0.4
	_heat_mat[GameMaterials.LAVA] = 0.8
	_heat_mat[GameMaterials.LAVA_STATIC] = 0.8   # cave pools and the lava sea (bug report 2026-10-28_08-26-42)


# ---------------- the rules (static, headless tested) ----------------

## Environment temperature (C) from the climate (-1..1), the calendar's day of the year and time of day
## (0 midnight), how far under the surface (m), and whether in water / in a room, plus heat (C).
static func environment_c(climate: float, day_of_year: int, year_days: int, month_days: int, time: float, below_m: float,
		in_water: bool, room: bool, heat_c: float) -> float:
	var t := 12.0 + 26.0 * climate
	t += -7.0 * cos(TAU * (day_of_year - 0.5 * month_days) / maxf(year_days, 1))   # coldest mid-January
	t += 3.5 * -cos(TAU * (time - 0.08)) - 0.5                                      # 2 am -4 C, 2 pm +3 C
	if below_m > 4.0:
		t = lerpf(t, Settings.cave_c, clampf((below_m - 4.0) / 4.0, 0.0, 1.0))
	if in_water:
		t += Settings.water_c
	if room:
		t += (Settings.room_c - t) * 0.5
	return t + heat_c * (1.5 if room else 0.7)


## Where the body temperature heads in an environment of env (C).
static func body_target(env: float) -> float:
	if env < Settings.comfort_min_c:
		return 37.0 - (Settings.comfort_min_c - env) * 0.25
	if env > Settings.comfort_max_c:
		return 37.0 + (env - Settings.comfort_max_c) * 0.2
	return 37.0


## The body after dt seconds (water draws heat twice as fast).
static func body_step(body: float, env: float, dt: float, in_water: bool) -> float:
	var k := minf(1.0, dt * (2.0 if in_water else 1.0) / Settings.body_ease_s)
	return body + (body_target(env) - body) * k


## Hunger points one second costs: faster when moving fast and when cold.
static func hunger_rate(fast: bool, cold: bool) -> float:
	return 1.0 / Settings.hunger_drain_s * (Settings.hunger_fast_mult if fast else 1.0) * (Settings.hunger_cold_mult if cold else 1.0)


## How fast food spoils at temp (C): 1 at 16 C, 2 at 32, a tenth when freezing.
static func spoil_rate(temp: float) -> float:
	return 0.1 if temp <= 0.0 else clampf(temp / 16.0, 0.1, 2.0)


## Ages every perishable stack of inv by days at rate; rotten ones become rot. Returns how many rotted.
static func age_inventory(inv: Inventory, days: float, rate: float) -> int:
	var rotted := 0
	for i in inv.size():
		var s: Dictionary = inv.get_slot(i)
		if s.is_empty() or not s.has("spoil"):
			continue
		var def := inv.items.get_def(s["id"])
		var sd := float(def.get("spoil_days", 0.0))
		if sd <= 0.0:
			continue
		var k := Settings.preserved_spoil_mult if "preserved" in def.get("tags", []) else 1.0   # salted, dried, smoked (part F)
		var sp := float(s["spoil"]) + days * rate * k / sd
		if sp >= 1.0:
			inv.set_slot(i, {"id": "rot", "count": s["count"]})
			rotted += 1
		else:
			s["spoil"] = sp
	return rotted


static func freshness_name(spoil: float) -> String:
	if spoil >= 0.8:
		return "Rotting"
	if spoil >= 0.5:
		return "Stale"
	return "Fresh"


# ---------------- the frame ----------------

func _process(delta: float) -> void:
	if player == null or main.world == null:
		return
	_probe_t -= delta
	if _probe_t <= 0.0:
		_probe_t = Settings.climate_probe_s
		_measure()
	var survival := GameMode.rule(player.game_mode, "takes_damage")
	var flat := Vector2(player.velocity.x, player.velocity.z).length()
	var fast := flat > Settings.m2v(Settings.walk_speed_m) * 1.15 or (player.in_liquid and flat > 1.0)
	body_c = body_step(body_c, felt_c(), delta, player.in_liquid)
	player.speed_mult = Settings.shiver_speed if survival and body_c < Settings.shiver_c else 1.0
	if survival:
		_hunger_step(delta, fast)
		_harm_step(delta)
		nutrition.drain(delta, hunger_rate(fast, body_c < Settings.cold_c) * Settings.hunger_drain_s)
	_health_t -= delta
	if _health_t <= 0.0:
		_health_t = 1.0
		apply_max_health()
	_spoil_t -= delta
	if _spoil_t <= 0.0:
		_spoil_t = 2.0
		_age_food()


func _hunger_step(delta: float, fast: bool) -> void:
	var cost := hunger_rate(fast, body_c < Settings.cold_c) * delta
	if saturation > 0.0:
		var s := minf(saturation, cost)
		saturation -= s
		cost -= s
	hunger = maxf(0.0, hunger - cost)
	var inv: PlayerInventory = player.inventory
	if hunger <= 0.0:
		_starve_t += delta
		if _starve_t >= Settings.starve_s:
			_starve_t = 0.0
			inv.damage(1, "starvation")   # starving kills, like Vintage Story (the developer's call)
			stats["starved"] += 1
			if inv.is_dead():
				player.died.emit()
	else:
		_starve_t = 0.0
	if hunger >= Settings.regen_hunger and inv.health < inv.max_health:
		_regen_t += delta
		if _regen_t >= Settings.regen_s:
			_regen_t = 0.0
			inv.heal(1)
			hunger = maxf(0.0, hunger - 0.5)
	else:
		_regen_t = 0.0


func _harm_step(delta: float) -> void:
	var every := 0.0
	if body_c < Settings.freeze_c:
		every = Settings.freeze_s
	elif body_c > Settings.heatstroke_c:
		every = Settings.heatstroke_s
	if every <= 0.0:
		_hurt_t = 0.0
		return
	_hurt_t += delta
	if _hurt_t >= every:
		_hurt_t = 0.0
		player.inventory.damage(1, "hypothermia" if body_c < Settings.freeze_c else "heatstroke")
		stats["froze" if body_c < Settings.freeze_c else "overheated"] += 1
		if player.inventory.is_dead():
			player.died.emit()


## Shelter, heat and the temperature where the player stands.
func _measure() -> void:
	var t0 := Time.get_ticks_usec()
	var p := player.camera.global_position
	var cell := Vector3i(p.floor())
	var gen: FineGen = main.world.generator.gen
	var climate := float(gen.column_info(cell.x, cell.z)["temperature"])
	underground_m = maxf(0.0, Settings.v2m(main.world.surface_height(cell.x, cell.z) - p.y))
	var cal = main.atmosphere.calendar
	var h_cells := 0.0
	in_room = false
	var bubble: SimBubble = main.bubble
	if bubble != null and bubble.sim != null:
		var r: Dictionary = probe.probe(bubble.sim, cell - bubble.origin * Settings.CHUNK, Vector3i(64, 32, 64), 65536, _open, _heat_mat,
			int(Settings.m2v(2.5)), Settings.m2v(1.0))
		in_room = bool(r["enclosed"])
		h_cells = float(r["heat"])
	heat = h_cells + _entity_heat(p)
	env_c = environment_c(climate, cal.day_of_year(), cal.year_days(), cal.month_days, cal.time, underground_m,
		player.in_liquid, in_room, heat * HEAT_C)
	outside_c = environment_c(climate, cal.day_of_year(), cal.year_days(), cal.month_days, cal.time, 0.0, false, false, 0.0)   # the minimap's line
	stats["probe_ms_max"] = maxf(stats["probe_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)


## Heat from lit firepits, furnaces, kilns, torches near p.
func _entity_heat(p: Vector3) -> float:
	var h := 0.0
	var r := Settings.m2v(ENTITY_HEAT_M)
	var f2 := Settings.m2v(2.0) * Settings.m2v(2.0)
	for e in main.blocks._by_pos.values():
		if not e.has_method("is_lit") or not (e.get("pos") is Vector3i) or not e.is_lit():
			continue
		var d2 := (Vector3(e.pos) + Vector3(0.5, 0.5, 0.5)).distance_squared_to(p)
		if d2 > r * r:
			continue
		h += float(ENTITY_HEAT.get(String(e.get("kind")), 4.0)) / (1.0 + d2 / f2)
	return h


## Ages the food in the player's inventory and in every container by the game time since last time.
func _age_food() -> void:
	var cal = main.atmosphere.calendar
	var now: float = cal.total()
	if _last_days < 0.0 or now < _last_days or now - _last_days > 2.0:
		_last_days = now   # first call, or the date jumped (/date, /time): nothing aged
		return
	var days := now - _last_days
	_last_days = now
	if days <= 0.0:
		return
	var t0 := Time.get_ticks_usec()
	var inv: PlayerInventory = player.inventory
	var rate := spoil_rate(env_c)
	var n := age_inventory(inv.inv, days, rate)
	if inv.has_bag():
		n += age_inventory(inv.bag, days, rate)
	if not inv.cursor.is_empty() and inv.cursor.has("spoil"):
		var sd := float(inv.items.get_def(inv.cursor["id"]).get("spoil_days", 0.0))
		if sd > 0.0:
			inv.cursor["spoil"] = minf(1.0, float(inv.cursor["spoil"]) + days * rate / sd)
	var gen: FineGen = main.world.generator.gen
	for e in main.blocks._by_pos.values():
		var c: Variant = e.get("inv")
		if not (c is Inventory) or not (e.get("pos") is Vector3i):
			continue
		var at: Vector3i = e.pos
		var below := Settings.v2m(main.world.surface_height(at.x, at.z) - at.y)
		var outdoor := Settings.cellar_c
		if below < 3.0:
			var cl := float(gen.column_info(at.x, at.z)["temperature"])
			outdoor = environment_c(cl, cal.day_of_year(), cal.year_days(), cal.month_days, cal.time, 0.0, false, false, 0.0)
		var room := below >= Settings.root_cellar_m and container_in_room(at)
		n += age_inventory(c, days, Seasons.container_rate(below, room, outdoor, String(e.get("kind"))))
	stats["rotted"] += n
	stats["spoil_ms_max"] = maxf(stats["spoil_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)


# ---------------- eating ----------------

## Can the player eat now (the bar not full, or hurt)?
func wants_food() -> bool:
	return hunger < Settings.hunger_max - 0.5 or player.inventory.health < player.inventory.max_health


var last_meal := {}   # the groups the last food filled (chat, tests)


## Eating `def` at `spoil`: hunger and saturation (stale food half), and its food groups.
func eat(def: Dictionary, spoil: float) -> void:
	var f: Dictionary = def.get("food", {})
	var k := 0.5 if spoil >= 0.5 else 1.0
	hunger = minf(Settings.hunger_max, hunger + float(f.get("hunger", 1)) * k)
	saturation = minf(hunger, saturation + float(f.get("saturation", 0)) * k)
	stats["eaten"] += 1
	last_meal = nutrition.eat(def, spoil)
	apply_max_health()


func reset() -> void:
	hunger = Settings.hunger_max
	saturation = 5.0
	body_c = 37.0
	nutrition.reset()
	apply_max_health()


func status() -> String:
	if body_c < Settings.freeze_c:
		return "Freezing"
	if body_c < Settings.shiver_c:
		return "Shivering"
	if body_c < Settings.cold_c:
		return "Cold"
	if body_c > Settings.heatstroke_c:
		return "Heatstroke"
	if body_c > Settings.hot_c:
		return "Hot"
	return ""


## The temperature the body feels: the air's, warmed by clothing in the cold (ADR 0059).
func felt_c() -> float:
	return Clothing.felt_c(env_c, clothing.warmth_c()) if clothing else env_c


func to_dict() -> Dictionary:
	return {"hunger": hunger, "saturation": saturation, "body_c": body_c, "nutrition": nutrition.to_dict(),
		"health": player.inventory.health if player != null else 0}


func load_dict(d: Dictionary) -> void:
	hunger = clampf(float(d.get("hunger", Settings.hunger_max)), 0.0, Settings.hunger_max)
	saturation = clampf(float(d.get("saturation", 5.0)), 0.0, hunger)
	body_c = clampf(float(d.get("body_c", 37.0)), 25.0, 45.0)
	var nd: Variant = d.get("nutrition", {})
	nutrition.load_dict(nd if nd is Dictionary else {})
	apply_max_health()
	if player != null and d.has("health"):   # the save may hold more than the base 20 (a balanced diet)
		var inv: PlayerInventory = player.inventory
		inv.health = clampi(int(d["health"]), 1, inv.max_health)
		inv.health_changed.emit(inv.health)


func snapshot() -> Dictionary:
	return {"hunger": snappedf(hunger, 0.01), "saturation": snappedf(saturation, 0.01), "body_c": snappedf(body_c, 0.01),
		"env_c": snappedf(env_c, 0.1), "felt_c": snappedf(felt_c(), 0.1), "in_room": in_room, "heat": snappedf(heat, 0.01), "underground_m": snappedf(underground_m, 0.1),
		"status": status(), "nutrition": nutrition.snapshot(),
		"max_health": player.inventory.max_health if player != null else 0, "stats": stats}


## Max health from the diet (ADR 0061); health above a lowered max is cut.
func apply_max_health() -> void:
	if player == null:
		return
	var inv: PlayerInventory = player.inventory
	var m := nutrition.max_health(Settings.max_health)
	if m == inv.max_health:
		return
	inv.max_health = m
	if inv.health > m:
		inv.health = m
	inv.health_changed.emit(inv.health)


## Is the voxel cell in a closed room (the ClimateProbe fill, no heat)? False outside the sim window.
func room_at(cell: Vector3i) -> bool:
	var bubble: SimBubble = main.bubble if main != null else null
	if bubble == null or bubble.sim == null:
		return false
	var r: Dictionary = probe.probe(bubble.sim, cell - bubble.origin * Settings.CHUNK, Vector3i(64, 32, 64), 65536, _open, PackedFloat32Array(), 0, 1.0)
	return bool(r["enclosed"])


## Does the container at `at` (its footprint corner) stand in a closed room? Tried from the cells beside
## and above it, kept for ROOM_CACHE_S.
func container_in_room(at: Vector3i) -> bool:
	var hit: Array = _rooms.get(at, [])
	if not hit.is_empty() and Inventory.clock_s < float(hit[1]):
		return hit[0]
	var b := Settings.BLOCK
	var room := false
	for o in [Vector3i(b / 2, b, b / 2), Vector3i(-1, 1, b / 2), Vector3i(b, 1, b / 2), Vector3i(b / 2, 1, -1), Vector3i(b / 2, 1, b)]:
		var cell: Vector3i = at + o
		if main.world.get_voxel(cell) == GameMaterials.AIR:
			room = room_at(cell)
			break
	if _rooms.size() > 512:
		_rooms.clear()
	_rooms[at] = [room, Inventory.clock_s + ROOM_CACHE_S]
	return room
