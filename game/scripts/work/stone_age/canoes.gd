class_name Canoes
extends Node3D

## Dugout canoes on the water (ADR 0059; the developer's call: a boat with momentum). A dugout item
## right clicked at water launches one; a right click near a canoe gets in (the player sits in it
## like on a chair, jump or sneak gets out); sneak + right click with an empty hand takes it back as
## the item. Seated, W/S paddle (thrust along the bow), A/D turn; it glides on and slows with water
## drag, rides the water surface it is over, a river's current pushes it downstream, and it stops dead
## against land (it never leaves the water). Canoes are saved with the world (StoneAge.world_dict).

const LEN_CELLS := 40       # 2.5 m in 6.25 cm work cells
const WIDE_CELLS := 8       # 0.5 m
const HIGH_CELLS := 4       # 0.25 m

var main: Node
var boats: Array = []       # {node, pos (world, voxels), yaw, vel (Vector3, voxels/s), spin}
var riding := -1            # index in boats of the one the player sits in
var stats := {"launched": 0, "boarded": 0, "paddled_m": 0.0, "max_speed_m": 0.0, "beached": 0}
static var _mesh: ArrayMesh


func setup(p_main: Node) -> void:
	main = p_main


static func mesh() -> ArrayMesh:
	if _mesh:
		return _mesh
	var size := Vector3i(WIDE_CELLS, HIGH_CELLS, LEN_CELLS)
	var c := WorkPalette.new_cells(size)
	for z in LEN_CELLS:
		var taper := 0 if z > 3 and z < LEN_CELLS - 4 else (4 - mini(z, LEN_CELLS - 1 - z)) / 2 + 1
		for x in range(taper, WIDE_CELLS - taper):
			for y in HIGH_CELLS:
				var wall := x == taper or x == WIDE_CELLS - 1 - taper or y == 0 or z < 3 or z > LEN_CELLS - 4
				if wall:
					c[WorkPalette.idx(size, x, y, z)] = (WorkPalette.WOOD if y > 0 else WorkPalette.WOOD_DARK) + 1
				elif y == 1:
					c[WorkPalette.idx(size, x, y, z)] = WorkPalette.TAR + 1   # the charred hollow
	_mesh = WorkPalette.mesh(c, size)
	return _mesh


func _make_node() -> Node3D:
	var root := Node3D.new()
	var mi := MeshInstance3D.new()
	mi.mesh = mesh()
	mi.material_override = WorkPalette.material()
	# the mesh spans (1, 0.5, 5) blocks from its corner: centre it on the boat's origin
	mi.scale = Vector3.ONE * Settings.BLOCK
	mi.position = -Vector3(0.5, 0.25, 2.5) * Settings.BLOCK + Vector3(0, 0.1 * Settings.BLOCK, 0)
	root.add_child(mi)
	add_child(root)
	return root


## The top of the water column at (x, z) near height y (the first air above water), or NAN.
func water_top(p: Vector3) -> float:
	var x := int(floor(p.x))
	var z := int(floor(p.z))
	var y0 := int(floor(p.y))
	for dy in range(12, -6, -1):
		var y := y0 + dy
		if WorkSystem._is_water(main.get_world_voxel(Vector3i(x, y, z))) and not WorkSystem._is_water(main.get_world_voxel(Vector3i(x, y + 1, z))):
			return float(y + 1)
	return NAN


func launch(at: Vector3, yaw: float) -> int:
	var top := water_top(at)
	if is_nan(top):
		return -1
	var b := {"node": _make_node(), "pos": Vector3(at.x, top, at.z), "yaw": yaw, "vel": Vector3.ZERO, "spin": 0.0}
	boats.append(b)
	_place(b)
	stats["launched"] += 1
	return boats.size() - 1


func _place(b: Dictionary) -> void:
	var n: Node3D = b["node"]
	n.position = b["pos"]
	n.rotation = Vector3(0, b["yaw"], 0)


## The boat whose hull the ray from `from` along `dir` passes within reach, or -1.
func aimed(from: Vector3, dir: Vector3) -> int:
	var best := -1
	var best_t := Settings.m2v(Settings.reach_m) + 1.0
	for i in boats.size():
		var b: Dictionary = boats[i]
		var to: Vector3 = b["pos"] - from
		var t := to.dot(dir)
		if t < 0.0 or t > best_t:
			continue
		var off: Vector3 = (from + dir * t) - b["pos"]
		var local := off.rotated(Vector3.UP, -float(b["yaw"]))
		if absf(local.x) < 0.5 * Settings.BLOCK + 0.5 and absf(local.z) < 2.5 * Settings.BLOCK and absf(local.y) < Settings.BLOCK:
			best = i
			best_t = t
	return best


## A right click: launch, board or take back. True when it took the click.
func use(held: Dictionary, hit: VoxelRaycastResult, from: Vector3, dir: Vector3) -> bool:
	var i := aimed(from, dir)
	if i >= 0:
		var sneaking: bool = main.stone_age._sneaking()
		if sneaking and held.is_empty():
			remove_boat(i)
			main.work._give([{"id": "dugout_canoe", "count": 1}], main.player.position)
			return true
		board(i)
		return true
	if not held.get("boat", false) or hit == null:
		return false
	var bed: Vector3i = main.stone_age._water_bed(hit)
	if bed.y < 0:
		main.work._say("Launch the canoe on water.")
		return true
	var yaw: float = main.player._yaw
	if launch(Vector3(bed) + Vector3(0.5, 1.0, 0.5), yaw) >= 0:
		if main.stone_age._consumes():
			main.player_inv.consume_held(1)
		main.work._note("launched", "dugout_canoe")
		main.work._say("Right click the canoe to get in.")
	return true


func board(i: int) -> void:
	riding = i
	var b: Dictionary = boats[i]
	main.player.sit(_seat_of(b), float(b["yaw"]) + PI)
	stats["boarded"] += 1
	main.work._note("paddled", "dugout_canoe")
	main.work._say("W and S paddle, A and D turn. Jump or sneak to get out.")


func _seat_of(b: Dictionary) -> Vector3:
	return b["pos"] + Vector3(0, 0.15 * Settings.BLOCK, 0)


func remove_boat(i: int) -> void:
	if riding == i:
		main.player.stand_up()
		riding = -1
	elif riding > i:
		riding -= 1
	boats[i]["node"].queue_free()
	boats.remove_at(i)


## The river's push at p in voxels/s: along the river's downhill direction where the generator says
## there is one (the slope of the water surface around p), else nothing.
func current_at(p: Vector3) -> Vector3:
	var info: Dictionary = main.world.generator.column_info(int(p.x), int(p.z))
	if float(info.get("river", 0.0)) < 0.05:
		return Vector3.ZERO
	var d := Settings.m2v(2.0)
	var g := Vector3.ZERO
	for v in [Vector3(d, 0, 0), Vector3(-d, 0, 0), Vector3(0, 0, d), Vector3(0, 0, -d)]:
		var h: float = float(main.world.generator.column_info(int(p.x + v.x), int(p.z + v.z)).get("height", 0))
		g -= v.normalized() * h
	g.y = 0.0
	return g.normalized() * Settings.m2v(Settings.canoe_current_m) if g.length() > 0.01 else Vector3.ZERO


func _physics_process(delta: float) -> void:
	if riding >= 0 and main.player.seat == Vector3.INF:
		riding = -1   # the player got out
	for i in boats.size():
		_step(i, delta)


## One boat for dt seconds: paddle thrust and turning (the rider's keys), current, drag, and the water
## surface it floats on; a bow that would run onto land stops it.
func _step(i: int, dt: float) -> void:
	var b: Dictionary = boats[i]
	var vel: Vector3 = b["vel"]
	var fwd := Vector3(-sin(float(b["yaw"])), 0, -cos(float(b["yaw"])))
	var paddle := 0.0
	var turn := 0.0
	if i == riding and not main.player.ui_open:
		paddle = Input.get_action_strength("move_forward") - Input.get_action_strength("move_back") * 0.5
		turn = Input.get_action_strength("move_left") - Input.get_action_strength("move_right")
	if i == riding and b.has("auto_paddle"):   # automation (stone_ages_test) paddles without keys
		paddle = float(b.get("auto_paddle", 0.0))
		turn = float(b.get("auto_turn", 0.0))
	vel += fwd * paddle * Settings.m2v(Settings.canoe_thrust_m) * dt
	b["spin"] = lerpf(float(b["spin"]), turn * Settings.canoe_turn_rad, minf(1.0, dt * 3.0))
	b["yaw"] = float(b["yaw"]) + float(b["spin"]) * dt
	b["cur_t"] = float(b.get("cur_t", 0.0)) - dt
	if float(b["cur_t"]) <= 0.0:
		b["cur_t"] = 0.5   # the generator is asked twice a second, not every frame
		b["cur"] = current_at(b["pos"])
	var cur: Vector3 = b.get("cur", Vector3.ZERO)
	if cur != Vector3.ZERO:
		vel += (cur - vel) * minf(1.0, dt * 0.15)
	vel *= exp(-Settings.canoe_drag * dt)
	# the hull slides sideways much less than it glides forwards (a keel)
	var side := Vector3(fwd.z, 0, -fwd.x)
	vel -= side * vel.dot(side) * minf(1.0, dt * 4.0)
	var next: Vector3 = b["pos"] + vel * dt
	var bow := next + fwd * 2.4 * Settings.BLOCK
	var top := water_top(next)
	if is_nan(top) or is_nan(water_top(bow)) or MobManager.is_solid(main.get_world_voxel(Vector3i(bow.floor()) + Vector3i.UP)):
		if vel.length() > 0.5:
			stats["beached"] += 1
		vel = Vector3.ZERO
	else:
		stats["paddled_m"] += Settings.v2m((next - b["pos"]).length()) if i == riding else 0.0
		b["pos"] = Vector3(next.x, lerpf(float(b["pos"].y), top, minf(1.0, dt * 4.0)), next.z)
	b["vel"] = vel
	stats["max_speed_m"] = maxf(stats["max_speed_m"], Settings.v2m(vel.length()))
	_place(b)
	if i == riding:
		main.player.seat = _seat_of(b)


func to_array() -> Array:
	var out := []
	for b in boats:
		var p: Vector3 = b["pos"]
		out.append({"pos": [p.x, p.y, p.z], "yaw": b["yaw"]})
	return out


func load_array(a: Array) -> void:
	for d in a:
		var p: Array = d.get("pos", [])
		if p.size() == 3:
			var b := {"node": _make_node(), "pos": Vector3(p[0], p[1], p[2]), "yaw": float(d.get("yaw", 0.0)), "vel": Vector3.ZERO, "spin": 0.0}
			boats.append(b)
			_place(b)


func summary() -> Dictionary:
	var s := stats.duplicate()
	s["boats"] = boats.size()
	s["riding"] = riding
	s["paddled_m"] = snappedf(s["paddled_m"], 0.1)
	s["max_speed_m"] = snappedf(s["max_speed_m"], 0.01)
	return s
