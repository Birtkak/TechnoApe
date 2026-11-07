class_name ToysTest
extends Node

## Debug (toys_test=1, ADR 0151): the movement toys through the real right click (Player._use) in
## survival. It builds a 26 m stone tower and a stone beam 12 m up beside the spawn, then
##   hook   fires the grappling hook at the beam (and once at a sand block: it must slip), reels in for
##          0.5 s, swings 4 s, lets go with the swing's speed
##   rope   hangs a climbing rope over the tower's edge, climbs it from the foot onto the top, takes it back
##   glide  walks off the tower top, opens the glider, flies level, turns, dives, pulls up, lands
## Each toy gets one sheet <out>/toys_<name>.png: a filmstrip (hook every 0.25 s from the side, rope 0.5 s
## and glide 0.7 s from behind) and a burst of 6 consecutive frames.
## One JSON line "debug toys {...}"; ok = every toy did its job and the body never stood inside a voxel.
## toys_only=glide,rope: just these.

const STRIP_EVERY_S := 0.25
const STRIP_MAX := 24

var main: Node
var p: Player
var r := {}
var _o := Vector3.ZERO      # the spawn's feet, on the ground
var _g := 0                 # the ground's voxel y there
var _inside := 0            # physics frames the body overlapped a voxel (a tunnel)
var _sheet: FrameSheet
var _strip_t := 0.0
var _filming := false
var _every := STRIP_EVERY_S


func run(p_main: Node) -> void:
	main = p_main
	p = main.player
	name = "ToysTest"
	main.add_child(self)
	var tree := get_tree()
	await tree.create_timer(1.0).timeout
	main.chat.send("/time set noon")
	main.chat.send("/gamemode survival")
	p.flying = false
	p.autopilot_action = "look"
	p.autopilot_speed_m = 0.0
	await tree.create_timer(2.0).timeout
	_o = (p.position + Vector3(0, 0, Settings.m2v(60.0))).floor()   # away from the spawn's monolith and its plaza
	_g = int(main.world.generator.gen.height_at(int(_o.x), int(_o.z))) + 1
	_o.y = _g
	p.teleport(_o + Vector3(0.5, 0.05, 0.5))
	await tree.create_timer(4.0).timeout   # the sim window follows the body before the build
	var only := ""
	for a in OS.get_cmdline_user_args():
		if a.begins_with("toys_only="):
			only = a.substr(10)
	_build()
	await tree.create_timer(1.0).timeout
	if only == "" or "hook" in only:
		await _hook()
	if only == "" or "rope" in only:
		await _rope()
	if only == "" or "glide" in only:
		await _glide()
	var ok := _inside == 0
	for k in r:
		ok = ok and bool(r[k].get("ok", false))
	r["inside_voxel_frames"] = _inside
	r["ok"] = ok
	print("debug toys " + JSON.stringify(r))
	if not DebugCapture._done:
		DebugCapture._finish()


func _physics_process(_delta: float) -> void:
	if p == null or p.body == null or not (main.toys.gliding or main.toys.climbing or main.toys.hook == "caught"):
		return   # only while a toy moves the body
	var b: AABB = p.body.aabb.grow(-0.3)   # shrunk: touching a floor, wall or ceiling is not a tunnel
	b.position += p.position
	if p.body._mover.intersects(b, main.world.terrain):
		if _inside == 0:
			var names := {}
			for x in range(int(floor(b.position.x)), int(ceil(b.end.x))):
				for y in range(int(floor(b.position.y)), int(ceil(b.end.y))):
					for z in range(int(floor(b.position.z)), int(ceil(b.end.z))):
						var m: int = main.get_world_voxel(Vector3i(x, y, z))
						if GameMaterials.blocks_bodies(m):
							names[GameMaterials.name_of(m)] = true
			print("toys: body inside a voxel at ", p.position, " ", names.keys(), " ", main.toys.snapshot())
		_inside += 1


func _process(delta: float) -> void:
	if not _filming:
		return
	_strip_t -= delta
	if _strip_t <= 0.0 and _sheet.count() < STRIP_MAX:
		_strip_t = _every
		var img := get_viewport().get_texture().get_image()
		if img != null and not img.is_empty():
			img.resize(_sheet.thumb_w, _sheet.thumb_h, Image.INTERPOLATE_BILINEAR)
			_sheet._frames.append(img)


func _m(m: float) -> int:
	return int(Settings.m2v(m))


func _box(lo: Vector3i, hi: Vector3i, mat: String) -> void:
	var n: int = main.bubble.fill_box(lo, hi, GameMaterials.id_of(mat))
	if n < 0:
		push_warning("toys_test: fill_box %s..%s failed" % [lo, hi])


## A 26 m tower 8 m to -x (4 m square), a 4 m stone beam 12 m up 10 m to +z, a sand block beside it.
func _build() -> void:
	var ox := int(_o.x)
	var oz := int(_o.z)
	_box(Vector3i(ox - _m(10), _g - _m(6), oz - _m(2)), Vector3i(ox - _m(6), _g + _m(26), oz + _m(2)), "stone")
	_box(Vector3i(ox - _m(2), _g + _m(12), oz + _m(9)), Vector3i(ox + _m(2), _g + _m(13), oz + _m(11)), "stone")
	_box(Vector3i(ox + _m(3), _g + _m(10), oz + _m(9)), Vector3i(ox + _m(4), _g + _m(11), oz + _m(11)), "sand_static")


func _hold(id: String) -> void:
	p.inventory.inv.set_slot(0, Inventory.make_stack(p.inventory.items, id, 1 if id != "climbing_rope" else 4))
	p.inventory.select(0)


func _aim_at(at: Vector3) -> void:
	p.set_perspective(0)   # aim from the eyes (a third-person camera sits 3.5 m back, out of reach)
	for _i in 2:
		var d: Vector3 = at - p.camera.global_position
		p.set_look(atan2(-d.x, -d.z), atan2(d.y, Vector2(d.x, d.z).length()))


var _side: Camera3D

## A camera standing at `from`, looking at `at` (the hook's swing and the rope climb are filmed from the side).
func _side_cam(from: Vector3, at: Vector3) -> void:
	if _side == null:
		_side = Camera3D.new()
		_side.far = p.camera.far
		main.add_child(_side)
	_side.position = from
	_side.look_at_from_position(from, at)
	_side.make_current()


func _film(on: bool, label: String, every_s := STRIP_EVERY_S) -> void:
	_every = every_s
	if on:
		p.set_perspective(1)   # filmed from behind: the body, the wing, the rope, the line
		_sheet = FrameSheet.new(256, 144, 6)
		_strip_t = 0.0
	elif _sheet != null:
		var tries := 0
		var want := _sheet.count() + 6
		while _sheet.count() < want and tries < 24:   # the burst: consecutive frames (a grab can come back empty: the next one)
			tries += 1
			await RenderingServer.frame_post_draw
			var img := get_viewport().get_texture().get_image()
			if img != null and not img.is_empty():
				img.resize(_sheet.thumb_w, _sheet.thumb_h, Image.INTERPOLATE_BILINEAR)
				_sheet._frames.append(img)
		_sheet.save(DebugCapture.out_dir + "/toys_%s.png" % label)
		p.camera.make_current()
	_filming = on


func _feet(at: Vector3) -> void:
	p.teleport(at)
	p.velocity = Vector3.ZERO
	await get_tree().create_timer(0.6).timeout


# ---------------- the toys ----------------

func _hook() -> void:
	var tree := get_tree()
	var res := {}
	_hold("grappling_hook")
	await _feet(Vector3(_o.x + 0.5, _g + 0.05, _o.z + 0.5))
	# sand: the hook must slip
	_aim_at(Vector3(_o.x + _m(3.5), _g + _m(10.5), _o.z + _m(10)))
	p.camera.force_update_transform()
	p._use(true)
	await tree.create_timer(0.6).timeout
	res["slipped_on_sand"] = int(main.toys.stats["hooks_slipped"]) > 0
	main.toys.unhook()
	# the beam's underside
	var anchor_aim := Vector3(_o.x + 0.5, _g + _m(12) - 0.2, _o.z + _m(10))
	_aim_at(anchor_aim)
	p._use(true)
	_film(true, "hook")
	_side_cam(Vector3(_o.x + _m(9), _g + _m(6), _o.z + _m(6)), Vector3(_o.x, _g + _m(6.5), _o.z + _m(6.5)))
	var t := 0.0
	while main.toys.hook == "flying" and t < 1.0:
		await tree.physics_frame
		t += get_physics_process_delta_time()
	res["caught"] = main.toys.hook == "caught"
	res["line_m"] = snappedf(Settings.v2m(main.toys._line), 0.1)
	p.autopilot_hold = true
	await tree.create_timer(0.5).timeout
	p.autopilot_hold = false
	res["line_after_reel_m"] = snappedf(Settings.v2m(main.toys._line), 0.1)
	res["lifted_m"] = snappedf(Settings.v2m(p.position.y - _g), 0.1)
	var zmin := INF
	var zmax := -INF
	var stretch := 0.0
	var vmax := 0.0
	t = 0.0
	var prev := 0.0
	while t < 8.0 and main.toys.hook == "caught":
		await tree.physics_frame
		t += get_physics_process_delta_time()
		var sp := Settings.v2m(main.toys._v.length())
		if t > 3.0 and sp < prev and prev > vmax * 0.8:
			break   # just past a swing's fastest point (its bottom): let go there
		prev = sp
		zmin = minf(zmin, p.position.z)
		zmax = maxf(zmax, p.position.z)
		if t > 1.0:
			vmax = maxf(vmax, Settings.v2m(p.velocity.length()))
		stretch = maxf(stretch, Settings.v2m((main.toys._anchor - main.toys._tie()).length() - main.toys._line))
	res["swing_z_m"] = snappedf(Settings.v2m(zmax - zmin), 0.1)
	res["swing_speed_max_m_s"] = snappedf(vmax, 0.1)
	res["line_stretch_max_m"] = snappedf(stretch, 0.01)
	var at := p.position
	res["let_go_speed_m_s"] = snappedf(Settings.v2m(main.toys._v.length()), 0.1)
	main.toys._let_go_with_swing()
	await tree.create_timer(1.0).timeout
	res["flew_after_let_go_m"] = snappedf(Settings.v2m(Vector2(p.position.x - at.x, p.position.z - at.z).length()), 0.1)
	await _film(false, "hook")
	res["ok"] = res["slipped_on_sand"] and res["caught"] and res["line_after_reel_m"] < res["line_m"] - 3.0 \
		and res["swing_z_m"] > 2.0 and res["line_stretch_max_m"] < 0.5 and res["flew_after_let_go_m"] > 3.0
	r["hook"] = res
	print("toys hook " + JSON.stringify(res))
	await tree.create_timer(1.5).timeout


func _rope() -> void:
	var tree := get_tree()
	var res := {}
	_hold("climbing_rope")
	var top := float(_g + _m(26))
	var edge_x := _o.x - _m(6)   # the tower's +x face
	await _feet(Vector3(edge_x - _m(1.0), top + 0.05, _o.z + 0.5))
	_aim_at(Vector3(edge_x - _m(0.3), top, _o.z + 0.5))   # the top face just short of the edge, looking +x
	p._use(true)
	res["hung"] = main.toys.ropes.size() == 1
	if not res["hung"]:
		res["ok"] = false
		r["rope"] = res
		return
	var rope: Dictionary = main.toys.ropes[0]
	res["length_m"] = snappedf(Settings.v2m(rope["top"] - rope["bottom"]), 0.1)
	res["left_in_hand"] = int(p.inventory.held().get("count", 0))
	# from its foot, facing the tower: walk forward = climb
	await _feet(Vector3(rope["x"] + _m(0.45), rope["bottom"] + 0.05, rope["z"]))
	p.set_look(PI / 2, 0.15)   # yaw 90 deg: forward is -x, into the tower
	_film(true, "rope", 0.5)
	p.autopilot_action = ""
	p.autopilot_speed_m = Settings.walk_speed_m
	var t := 0.0
	var climbed := false
	while t < 14.0:
		await tree.physics_frame
		t += get_physics_process_delta_time()
		climbed = climbed or main.toys.climbing
		if p.is_on_floor() and p.position.y >= top - 0.5:
			break
	p.autopilot_action = "look"
	p.autopilot_speed_m = 0.0
	res["climbed"] = climbed
	res["climb_s"] = snappedf(t, 0.1)
	res["on_top"] = p.is_on_floor() and p.position.y >= top - 0.5
	await tree.create_timer(0.5).timeout
	await _film(false, "rope")
	# take it back: aim at its middle from the top
	_aim_at(Vector3(rope["x"], rope["top"] - _m(0.1), rope["z"]))   # its knot, above the ledge
	p._use(true)
	res["taken_back"] = main.toys.ropes.is_empty()
	res["ok"] = res["climbed"] and res["on_top"] and res["taken_back"] and res["length_m"] > 15.0
	r["rope"] = res
	print("toys rope " + JSON.stringify(res))


func _glide() -> void:
	var tree := get_tree()
	var res := {}
	_hold("glider")
	var top := float(_g + _m(26))
	var health: int = p.inventory.health
	await _feet(Vector3(_o.x - _m(8), top + 0.05, _o.z - _m(1.5)))
	p.set_look(0.0, -0.1)   # forward -z, toward the edge 0.5 m away
	_film(true, "glide", 0.7)
	p.autopilot_action = ""
	p.autopilot_speed_m = Settings.walk_speed_m
	var t := 0.0
	while p.velocity.y > -Settings.m2v(2.0) and t < 3.0:
		await tree.physics_frame
		t += get_physics_process_delta_time()
	p.autopilot_action = "look"
	p.autopilot_speed_m = 0.0
	p._use(true)   # the right click opens it
	p.set_look(p._yaw, 0.0)
	res["opened"] = main.toys.gliding
	# level 1.5 s (the glide ratio from 0.5 s on), then a left turn, a dive, a pull-up, level to the ground
	var a := Vector3.INF
	var vmax := 0.0
	var dive_low := INF
	var climb_high := -INF
	var roll_max := 0.0
	t = 0.0
	while main.toys.gliding and t < 20.0:
		await tree.physics_frame
		var dt := get_physics_process_delta_time()
		t += dt
		if t >= 0.5 and a == Vector3.INF:
			a = p.position
		if t >= 1.5 and not res.has("glide_ratio"):
			res["glide_ratio"] = snappedf(Vector2(p.position.x - a.x, p.position.z - a.z).length() / maxf(a.y - p.position.y, 0.01), 0.1)
			res["trim_speed_m_s"] = snappedf(main.toys._gs, 0.1)
		if t > 1.5:
			p.set_look(p._yaw + 0.7 * dt, p._pitch)
		if t > 3.0 and t < 3.8:
			p.set_look(p._yaw, -0.9)
		elif t >= 3.8 and t < 5.3:
			p.set_look(p._yaw, 0.35)
		elif t >= 5.3:
			p.set_look(p._yaw, -0.05)
		if t > 3.0 and t < 3.8:
			vmax = maxf(vmax, main.toys._gs)
			dive_low = p.position.y
		if t >= 3.8 and t < 5.3:
			climb_high = maxf(climb_high, p.position.y)
		roll_max = maxf(roll_max, absf(main.toys.roll))
	res["dive_speed_m_s"] = snappedf(vmax, 0.1)
	res["pull_up_gain_m"] = snappedf(Settings.v2m(climb_high - dive_low), 0.1)
	res["roll_max_rad"] = snappedf(roll_max, 0.01)
	res["flight_s"] = snappedf(t, 0.1)
	await tree.create_timer(0.5).timeout
	res["landed"] = not main.toys.gliding and (p.is_on_floor() or p.in_liquid)
	res["damage"] = maxi(0, health - p.inventory.health)
	await _film(false, "glide")
	res["ok"] = res["opened"] and float(res.get("glide_ratio", 0.0)) > 5.0 and float(res.get("glide_ratio", 0.0)) < 12.0 \
		and res["dive_speed_m_s"] > Settings.glider_trim_speed_m * 1.4 and res["pull_up_gain_m"] > 1.0 and res["landed"] and res["roll_max_rad"] > 0.1
	r["glide"] = res
	print("toys glide " + JSON.stringify(res))
