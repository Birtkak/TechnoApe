class_name Megaliths
extends Node

## Megaliths and stone circles (ADR 0059). The road, all by hand in the world:
## 1. Quarry: drive WEDGES wooden wedges (right clicks) into the top of bare rock where three blocks of
##    it lie in a row along the way the player faces: the slab splits loose as a MegalithEntity.
## 2. Move: two logs under it (rollers) and two cordage round it (ropes), then lever strokes (right
##    clicks with a lever pole, held clicks repeat) push it PUSH_STROKES strokes per block away from
##    the player along its length or across it; it rides up a one-block step and needs ground under
##    its middle.
## 3. Raise: sneak + RAISE_STROKES lever strokes stand it on its end (the end nearest the player).
## 4. A circle: at least CIRCLE_MIN standing stones round a centre (each within RING_TOL of the mean
##    radius, which is at least RING_MIN_M), and a heel stone outside the ring whose direction from
##    the centre is within ALIGN_DEG of the summer solstice sunrise. A finished circle is a shrine
##    (the developer's call: a travel network): an empty hand on one of its stones sets the respawn
##    point there, and with two or more circles a picker sends the player to another one.
## 5. Standing in the circle when the sun rises on midsummer's day: the solstice.

const WEDGES := 6
const PUSH_STROKES := 6
const RAISE_STROKES := 12
const CIRCLE_MIN := 5
const RING_TOL := 0.35
const RING_MIN_M := 2.0
const ALIGN_DEG := 15.0
const CHECK_M := 14.0

var main: Node
var circles: Array = []      # {centre: Vector3, heel: Vector3, stones: [anchors], name}
var _wedged := {}            # quarry anchor (Vector3i) -> wedges driven
var _sunrise := Vector2.ZERO # the summer solstice sunrise direction on the ground (x, z), cached
var _solstice_seen := -1     # the year the solstice was last seen
var stats := {"quarried": 0, "pushed": 0, "raised": 0, "circles": 0, "travels": 0, "solstices": 0}


func setup(p_main: Node) -> void:
	main = p_main


# ---------------- the sky ----------------

## The summer solstice: the day of the year with the highest noon sun, and the direction (x, z) the
## sun rises in on it. From the calendar's real sun geometry, so it follows its latitude.
func solstice() -> Dictionary:
	var cal = main.atmosphere.calendar
	var best_d := 0
	var best_y := -2.0
	var base: int = cal.day - cal.day_of_year()
	for d in cal.year_days():
		var y: float = cal.sun_dir_at(float(base + d) + 0.5).y
		if y > best_y:
			best_y = y
			best_d = d
	var t0 := float(base + best_d)
	var lo := t0
	var hi := t0 + 0.5
	for i in 30:
		var mid := (lo + hi) * 0.5
		if cal.sun_dir_at(mid).y < 0.0:
			lo = mid
		else:
			hi = mid
	var s: Vector3 = cal.sun_dir_at(hi)
	return {"day_of_year": best_d, "sunrise": Vector2(s.x, s.z).normalized(), "time": hi - t0}


func sunrise_dir() -> Vector2:
	if _sunrise == Vector2.ZERO:
		_sunrise = solstice()["sunrise"]
	return _sunrise


# ---------------- clicks ----------------

## A right click not on a megalith: wedges into rock (quarrying). True when it took the click.
func use(held: Dictionary, hit: VoxelRaycastResult, _from: Vector3, dir: Vector3, first_press: bool) -> bool:
	if held.get("id", "") != "wooden_wedge" or hit == null or not first_press:
		return false
	if hit.previous_position != hit.position + Vector3i.UP:
		return false
	var a := BlockEntities.anchor_for(hit.position)
	a.y = hit.position.y - Settings.BLOCK + 1   # the top block of the rock
	var ax := _axis_of(dir)
	var blocks: Array[Vector3i] = []
	for i in MegalithEntity.LEN:
		blocks.append(a + ax * i * Settings.BLOCK)
	for b in blocks:
		for c in BlockEntities.cube(b):
			if main.stone_age._surface(main.get_world_voxel(c)) == "ground" or not MobManager.is_solid(main.get_world_voxel(c)):
				main.work._say("Wedges split a megalith out of bare rock: three blocks of it in a row, the way you face.")
				return true
	_wedged[a] = int(_wedged.get(a, 0)) + 1
	if main.stone_age._consumes():
		main.player_inv.consume_held(1)
	main.work._particles("block_hit", Vector3(hit.position) + Vector3(0.5, 1.0, 0.5), GameMaterials.STONE)
	if int(_wedged[a]) < WEDGES:
		main.work._say("A wedge in the crack (%d/%d)." % [_wedged[a], WEDGES])
		return true
	_wedged.erase(a)
	var e := make(a, "x" if ax.x != 0 else "z")
	stats["quarried"] += 1
	main.work._note("quarried", "standing_stone")
	main.work._say("The stone splits loose. Put two logs under it and tie two cordage round it, then lever it along.")
	return e != null


static func _axis_of(dir: Vector3) -> Vector3i:
	return Vector3i(int(signf(dir.x)), 0, 0) if absf(dir.x) >= absf(dir.z) else Vector3i(0, 0, int(signf(dir.z)))


## A new megalith at anchor `a` along `axis`: its cells become solid work voxels.
func make(a: Vector3i, axis: String, from: Dictionary = {}) -> MegalithEntity:
	var e: MegalithEntity = main.blocks.create(a, "megalith")
	if e == null:
		return null
	if not from.is_empty():
		e.load_from(from)
	e.axis = axis
	main.blocks._register_cells(e)
	for c in e.footprint_cells():
		main.set_world_voxel(c, GameMaterials.WORK_BLOCK)
	main.work._watch(e)
	return e


## A right click on a megalith with `held`.
func use_stone(e: MegalithEntity, held: Dictionary, first_press: bool, _from: Vector3, dir: Vector3) -> void:
	var id: String = held.get("id", "")
	var lever: bool = "lever" in held.get("tags", [])
	if not first_press and not lever:
		return
	if lever:
		if main.stone_age._sneaking():
			_lift(e, dir)
		else:
			_push(e, dir)
		return
	if id == "":
		var c := circle_of(e)
		if c >= 0:
			_shrine(e, c)
		else:
			main.work._say(_state(e))
		return
	if main.items.has_tag(id, "log") and not e.standing() and e.rollers < MegalithEntity.ROLLER_LOGS:
		e.rollers += 1
		if main.stone_age._consumes():
			main.player_inv.consume_held(main.items.units(id))   # a whole log block
		e.changed.emit()
		main.work._say(_state(e))
		return
	if id == "cordage" and not e.standing() and e.ropes < MegalithEntity.ROPES:
		e.ropes += 1
		if main.stone_age._consumes():
			main.player_inv.consume_held(1)
		e.changed.emit()
		main.work._say(_state(e))
		return
	main.work._say(_state(e))


func _state(e: MegalithEntity) -> String:
	if e.standing():
		return "A standing stone. Five or more in a ring, with a heel stone outside toward the midsummer sunrise, make a circle."
	if e.rollers < MegalithEntity.ROLLER_LOGS:
		return "Put logs under it as rollers (%d/%d)." % [e.rollers, MegalithEntity.ROLLER_LOGS]
	if e.ropes < MegalithEntity.ROPES:
		return "Tie cordage round it (%d/%d)." % [e.ropes, MegalithEntity.ROPES]
	return "Lever it along with a lever pole (right clicks); sneak to raise it on its end."


## A lever stroke: every PUSH_STROKES move it a block away from the player (a step of one block up
## where its way is blocked at its own height).
func _push(e: MegalithEntity, dir: Vector3) -> void:
	if not e.movable():
		main.work._say(_state(e))
		return
	e.push += 1
	main.work._wear_held()
	main.work._particles("block_hit", main.work._at(e.pos, Vector3(0.5, 0.1, 0.5)), GameMaterials.DIRT)
	if e.push < PUSH_STROKES:
		return
	e.push = 0
	var step := _axis_of(dir) * Settings.BLOCK
	for up in [0, Settings.BLOCK]:
		var to := e.pos + step + Vector3i(0, up, 0)
		if _room(e, to, e.axis) and _supported(e, to):
			_move(e, to, e.axis)
			stats["pushed"] += 1
			main.work._note("megalith_moved", "standing_stone")
			return
	main.work._say("It will not go further that way.")


## Sneak + lever strokes: it stands up on the end nearest the player.
func _lift(e: MegalithEntity, dir: Vector3) -> void:
	if e.standing():
		main.work._say("It already stands.")
		return
	if e.ropes < MegalithEntity.ROPES:
		main.work._say(_state(e))
		return
	e.lift += 1
	main.work._wear_held()
	if e.lift < RAISE_STROKES:
		if e.lift % 4 == 0:
			main.work._say("Raising (%d/%d)." % [e.lift, RAISE_STROKES])
		return
	e.lift = 0
	var bl := e.blocks()
	# the end nearest the player: the first block when the player looks along the stone's length
	var foot: Vector3i = bl[0] if Vector3(e.dir()).dot(dir) > 0.0 else bl[bl.size() - 1]
	if not _room(e, foot, "y"):
		main.work._say("There is no room to raise it here.")
		return
	_move(e, foot, "y")
	stats["raised"] += 1
	main.work._note("raised", "standing_stone")
	main.work._say("The stone stands.")
	_check_circles()


## Its blocks at anchor `to` along `axis` are free (air, plants, or its own cells).
func _room(e: MegalithEntity, to: Vector3i, axis: String) -> bool:
	var own := {}
	for c in e.footprint_cells():
		own[c] = true
	var d := Vector3i(1, 0, 0) if axis == "x" else (Vector3i(0, 1, 0) if axis == "y" else Vector3i(0, 0, 1))
	for i in MegalithEntity.LEN:
		for c in BlockEntities.cube(to + d * i * Settings.BLOCK):
			if own.has(c):
				continue
			var m: int = main.get_world_voxel(c)
			if m != GameMaterials.AIR and not (m > 0 and GameMaterials.defs()[m - 1].get("replaceable", false)):
				return false
	return true


## Ground under its middle block at anchor `to` (lying).
func _supported(e: MegalithEntity, to: Vector3i) -> bool:
	var mid := to + e.dir() * Settings.BLOCK
	for x in Settings.BLOCK:
		for z in Settings.BLOCK:
			if MobManager.is_solid(main.get_world_voxel(mid + Vector3i(x, -1, z))):
				return true
	return false


func _move(e: MegalithEntity, to: Vector3i, axis: String) -> MegalithEntity:
	var d := e.to_dict()
	main.work.remove_piece(e)
	return make(to, axis, d)


# ---------------- circles ----------------

func standing_stones() -> Array:
	return main.blocks.all_of("megalith").filter(func(s: MegalithEntity) -> bool: return s.standing())


## Looks for a new circle among the standing stones near the newest ones.
func _check_circles() -> void:
	var all := standing_stones()
	var known := {}
	for c in circles:
		for s in c["stones"]:
			known[s] = true
	var found := find_circle(all.map(func(s: MegalithEntity) -> Vector3: return Vector3(s.pos) + Vector3(0.5, 0, 0.5) * Settings.BLOCK), sunrise_dir())
	if found.is_empty():
		return
	var anchors := []
	for i in found["ring"] + [found["heel"]]:
		anchors.append(all[i].pos)
	for a in anchors:
		if known.has(a):
			return   # a circle already counted
	var c := {"centre": found["centre"], "heel": found["heel_pos"], "stones": anchors, "name": "Circle %d" % (circles.size() + 1)}
	circles.append(c)
	stats["circles"] += 1
	main.work._note("megalith", "standing_stone")
	main.work._say("A stone circle! Its heel stone points at the midsummer sunrise. An empty hand on its stones makes it home; with two circles you can travel between them.")


## A ring of at least CIRCLE_MIN of `points` round a centre, plus a heel stone toward `sunrise` (x, z):
## {centre, ring: [indices], heel: index, heel_pos} or {}. Static for the tests.
static func find_circle(points: Array, sunrise: Vector2) -> Dictionary:
	var n := points.size()
	if n < CIRCLE_MIN + 1:
		return {}
	for h in n:
		var ring := []
		for i in n:
			if i != h:
				ring.append(i)
		# the ring's centre: the mean of the other stones, then drop those far from the mean radius
		for pass_i in 3:
			if ring.size() < CIRCLE_MIN:
				break
			var centre := Vector3.ZERO
			for i in ring:
				centre += points[i]
			centre /= ring.size()
			var mean := 0.0
			for i in ring:
				mean += _flat(points[i] - centre).length()
			mean /= ring.size()
			var keep := ring.filter(func(i: int) -> bool: return absf(_flat(points[i] - centre).length() - mean) <= RING_TOL * mean)
			if keep.size() == ring.size():
				if mean < Settings.m2v(RING_MIN_M):
					break
				var to_heel := _flat(points[h] - centre)
				var dist := to_heel.length()
				if dist < mean * 1.3 or dist > mean * 4.0:
					break
				var ang := rad_to_deg(absf(Vector2(to_heel.x, to_heel.z).normalized().angle_to(sunrise)))
				if ang <= ALIGN_DEG:
					return {"centre": centre, "ring": ring, "heel": h, "heel_pos": points[h], "radius": mean, "angle_deg": ang}
				break
			ring = keep
	return {}


static func _flat(v: Vector3) -> Vector3:
	return Vector3(v.x, 0, v.z)


## The circle a stone belongs to, or -1.
func circle_of(e: MegalithEntity) -> int:
	for i in circles.size():
		if e.pos in circles[i]["stones"]:
			return i
	return -1


## An empty hand on a circle's stone: home is here; with other circles, a picker to travel.
func _shrine(e: MegalithEntity, c: int) -> void:
	var centre: Vector3 = circles[c]["centre"]
	main._spawn_pos = centre + Vector3(0, 0.2 * Settings.BLOCK, 0)
	main.work._note("shrine", "standing_stone")
	if circles.size() < 2:
		main.work._say("%s is your home now. Raise a second circle to travel between them." % circles[c]["name"])
		return
	var opts := []
	for i in circles.size():
		if i == c:
			continue
		var d: Vector3 = circles[i]["centre"] - centre
		opts.append({"id": "travel_%d" % i, "output": {"id": "standing_stone", "count": 1}, "circle": i,
			"line": "%s, %d m %s" % [circles[i]["name"], int(Settings.v2m(_flat(d).length())), _compass(d)]})
	main.work._picking = e
	e.set_meta("travel", self)
	main.player.set_ui_open(true, false)
	main.work.picker.open("Travel to", opts, main.items, func(r: Dictionary) -> String: return r["line"])
	if main.player.autopilot:
		main.work.picker.close()


static func _compass(d: Vector3) -> String:
	var a := fposmod(rad_to_deg(atan2(d.x, -d.z)), 360.0)
	return ["north", "northeast", "east", "southeast", "south", "southwest", "west", "northwest"][int(round(a / 45.0)) % 8]


## The picker chose a circle (MegalithEntity.set_recipe forwards here).
func travel(r: Dictionary) -> void:
	var i := int(r.get("circle", -1))
	if i < 0 or i >= circles.size():
		return
	var to: Vector3 = circles[i]["centre"] + Vector3(0, 0.2 * Settings.BLOCK, 0)
	stats["travels"] += 1
	main.work._note("travelled", "standing_stone")
	main.player.teleport(to)


## Midsummer sunrise inside a circle: the solstice (once a year).
func _process(_delta: float) -> void:
	if circles.is_empty() or Engine.get_process_frames() % 30 != 0:
		return
	var cal = main.atmosphere.calendar
	if cal.year() == _solstice_seen:
		return
	var s := solstice()
	if cal.day_of_year() != int(s["day_of_year"]) or absf(cal.time - float(s["time"])) > 0.06:
		return
	for c in circles:
		if _flat(main.player.position - c["centre"]).length() < Settings.m2v(CHECK_M):
			_solstice_seen = cal.year()
			stats["solstices"] += 1
			main.work._note("solstice", "standing_stone")
			main.work._say("The midsummer sun rises over the heel stone.")
			return


func to_dict() -> Dictionary:
	var out := []
	for c in circles:
		var ce: Vector3 = c["centre"]
		var he: Vector3 = c["heel"]
		out.append({"centre": [ce.x, ce.y, ce.z], "heel": [he.x, he.y, he.z], "name": c["name"],
			"stones": c["stones"].map(func(a: Vector3i) -> Array: return [a.x, a.y, a.z])})
	return {"circles": out, "solstice_seen": _solstice_seen}


func load_dict(d: Dictionary) -> void:
	circles.clear()
	for c in d.get("circles", []):
		var ce: Array = c.get("centre", [0, 0, 0])
		var he: Array = c.get("heel", [0, 0, 0])
		circles.append({"centre": Vector3(ce[0], ce[1], ce[2]), "heel": Vector3(he[0], he[1], he[2]), "name": String(c.get("name", "Circle")),
			"stones": c.get("stones", []).map(func(a: Array) -> Vector3i: return Vector3i(int(a[0]), int(a[1]), int(a[2])))})
	_solstice_seen = int(d.get("solstice_seen", -1))


func summary() -> Dictionary:
	var s := stats.duplicate()
	s["standing"] = standing_stones().size()
	s["lying"] = main.blocks.all_of("megalith").size() - s["standing"]
	s["circles_known"] = circles.size()
	return s
