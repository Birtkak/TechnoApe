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
## toys_only=glide,rope: just these. toys_world=1 instead finds the steepest real drop within 400 m of the
## spawn (generator heights, checked against the voxels) and glides off it, hangs a rope over it and climbs it,
## and hooks its face from the foot (sheets toys_world_glide|rope|hook.png, JSON "world").

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
var _settle := 0


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
	if "toys_world=1" in OS.get_cmdline_user_args():
		await _world()
		_finish()
		return
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
	_finish()


func _finish() -> void:
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
	if _settle > 0:
		_settle -= 1
		return
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
	_box(Vector3i(ox - _m(4), _g + _m(10), oz + _m(9)), Vector3i(ox - _m(3), _g + _m(11), oz + _m(11)), "leaves")


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
	_settle = 20   # a teleport onto a slope may start inside it: Player unsticks it, not a toy
	p.teleport(at)
	p.velocity = Vector3.ZERO
	await get_tree().create_timer(0.6).timeout
	var t := 0.0
	while p.edits_locked and t < 15.0:   # a far teleport is covered until the ground is in: no clicks before
		await get_tree().create_timer(0.25).timeout
		t += 0.25
	if t > 0.0:
		p.teleport(at)   # the cover may have dropped the body a little
		await get_tree().create_timer(0.4).timeout


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
	# leaves: it holds (a crown to swing from)
	_aim_at(Vector3(_o.x - _m(3.5), _g + _m(10.5), _o.z + _m(10)))
	p._use(true)
	await tree.create_timer(0.6).timeout
	res["caught_leaves"] = main.toys.hook == "caught" and "leaves" in GameMaterials.name_of(main.get_world_voxel(main.toys._anchor_voxel))
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
	res["ok"] = res["slipped_on_sand"] and res["caught_leaves"] and res["caught"] and res["line_after_reel_m"] < res["line_m"] - 3.0 \
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
	# hung again, its anchor dug away: it falls as an item
	_aim_at(Vector3(edge_x - _m(0.3), top, _o.z + 0.5))
	p._use(true)
	var anchor = main.toys.ropes[0].get("anchor") if main.toys.ropes.size() == 1 else null
	if anchor != null:
		main.bubble.set_voxel(anchor, 0)
		await tree.create_timer(1.0).timeout
	res["falls_when_dug"] = anchor != null and main.toys.ropes.is_empty()
	res["ok"] = res["climbed"] and res["on_top"] and res["taken_back"] and res["falls_when_dug"] and res["length_m"] > 15.0
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


# ---------------- on real terrain (toys_world=1) ----------------

## The highest real ground voxel's top in the column (x, z), searched down from `from_y`.
func _ground_y(x: int, z: int, from_y: int) -> int:
	for y in range(from_y, from_y - _m(80), -1):
		if GameMaterials.blocks_bodies(main.get_world_voxel(Vector3i(x, y, z))):
			return y + 1
	return from_y - _m(80)


## The same, past trees: leaves and wood are not the ground a rim is made of.
func _terrain_y(x: int, z: int, from_y: int) -> int:
	for y in range(from_y, from_y - _m(80), -1):
		var m: int = main.get_world_voxel(Vector3i(x, y, z))
		var n := GameMaterials.name_of(m)
		if GameMaterials.blocks_bodies(m) and not ("leaves" in n or "log" in n or "wood" in n or "bark" in n):
			return y + 1
	return from_y - _m(80)


func _heal() -> void:
	p.inventory.health = p.inventory.max_health


func _world() -> void:
	var tree := get_tree()
	var gen = main.world.generator.gen
	var sx := int(p.position.x)
	var sz := int(p.position.z)
	var best := {"drop": 0}
	var step := _m(4.0)
	for ix in range(-100, 101):
		for iz in range(-100, 101):
			if ix * ix + iz * iz < 100:
				continue   # 40 m: off the monolith and its plaza
			var x := sx + ix * step
			var z := sz + iz * step
			var h: int = gen.height_at(x, z)
			for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
				var near: int = h - int(gen.height_at(x + d.x * _m(3.0), z + d.y * _m(3.0)))
				var far: int = h - int(gen.height_at(x + d.x * _m(15.0), z + d.y * _m(15.0)))
				if near > best["drop"] and far >= near:
					best = {"x": x, "z": z, "h": h, "dx": d.x, "dz": d.y, "drop": near}
	var w := {"site": [best.get("x"), best.get("h"), best.get("z")], "gen_drop_3m": snappedf(Settings.v2m(best["drop"]), 0.1)}
	r["world"] = w
	if int(best["drop"]) < _m(6.0):
		w["ok"] = false
		return
	var dir := Vector3(best["dx"], 0, best["dz"])
	var top := Vector3(best["x"] + 0.5, best["h"] + _m(2.0), best["z"] + 0.5)
	await _feet(top)
	await tree.create_timer(5.0).timeout   # the cliff and the land below stream in
	# the rim: walk out along dir while the terrain (generator heights: no tree crowns) holds within 1 m
	var gy := _terrain_y(int(top.x), int(top.z), int(top.y) + _m(8))
	var edge := Vector3(top.x, gy, top.z)
	for _i in _m(12.0):
		var nx := edge + dir
		var ny := _terrain_y(int(nx.x), int(nx.z), gy + _m(3))
		if ny < gy - _m(1.0):
			break
		edge = Vector3(nx.x, ny, nx.z)
		gy = ny
	var below := edge + dir * _m(4.0)
	w["real_drop_4m"] = snappedf(Settings.v2m(edge.y - _ground_y(int(below.x), int(below.z), int(edge.y))), 0.1)
	var yaw := atan2(-dir.x, -dir.z)
	var start := edge - dir * _m(0.5)
	start.y = _terrain_y(int(start.x), int(start.z), int(edge.y) + _m(3)) + 0.05
	# ---- glide off the rim, looking a little down, straight on to the landing ----
	_hold("glider")
	_heal()
	await _feet(start)
	p.set_look(yaw, -0.12)
	_film(true, "world_glide", 0.7)
	await tree.create_timer(0.5).timeout
	p.teleport(edge + dir * _m(1.0) + Vector3(0, _m(1.2), 0))   # a run and a jump off the rim (a lip stands on this one)
	p.velocity = dir * Settings.m2v(Settings.walk_speed_m)
	var t := 0.0
	while p.velocity.y > -Settings.m2v(2.0) and t < 2.0:
		await tree.physics_frame
		t += get_physics_process_delta_time()
	p._use(true)
	p.set_look(yaw, -0.12)
	var g := {"opened": main.toys.gliding}
	var from := p.position
	var snags := 0
	t = 0.0
	while main.toys.gliding and t < 25.0:
		await tree.physics_frame
		t += get_physics_process_delta_time()
		if p.body.blocked.x > 0.0 or p.body.blocked.z > 0.0:
			snags += 1
	await tree.create_timer(0.6).timeout
	g["flight_s"] = snappedf(t, 0.1)
	g["dist_m"] = snappedf(Settings.v2m(Vector2(p.position.x - from.x, p.position.z - from.z).length()), 0.1)
	g["height_lost_m"] = snappedf(Settings.v2m(from.y - p.position.y), 0.1)
	g["snag_frames"] = snags
	g["damage"] = p.inventory.max_health - p.inventory.health
	g["landed"] = not main.toys.gliding and (p.is_on_floor() or p.in_liquid)
	await _film(false, "world_glide")
	g["ok"] = g["opened"] and g["landed"] and g["damage"] == 0 and g["dist_m"] > 10.0
	w["glide"] = g
	print("toys world glide " + JSON.stringify(g))
	# ---- a rope over the rim, climbed from its foot ----
	_hold("climbing_rope")
	_heal()
	await _feet(start)
	_aim_at(edge - dir * _m(0.3) + Vector3(0.0, 0.0, 0.0))
	var ro := {"hung": false}
	var aim := p._aim()
	ro["aimed"] = [GameMaterials.name_of(main.get_world_voxel(aim.position)), aim.previous_position - aim.position] if aim else "nothing in reach"
	p._use(true)
	if main.toys.ropes.size() > 0:
		var rope: Dictionary = main.toys.ropes[-1]
		ro["hung"] = true
		ro["length_m"] = snappedf(Settings.v2m(rope["top"] - rope["bottom"]), 0.1)
		await _feet(Vector3(rope["x"], rope["bottom"] + 0.05, rope["z"]) + dir * _m(0.45))
		p.set_look(yaw + PI, 0.15)
		_film(true, "world_rope", 0.5)
		p.autopilot_action = ""
		p.autopilot_speed_m = Settings.walk_speed_m
		var climbed := false
		var stuck := 0.0
		var last_y := p.position.y
		t = 0.0
		while t < 25.0:
			await tree.physics_frame
			var dt := get_physics_process_delta_time()
			t += dt
			climbed = climbed or main.toys.climbing
			stuck = stuck + dt if absf(p.position.y - last_y) < 0.01 and main.toys.climbing else 0.0
			last_y = p.position.y
			if stuck > 1.0 or (p.is_on_floor() and p.position.y >= rope["top"] - _m(0.6)):
				break
		p.autopilot_action = "look"
		p.autopilot_speed_m = 0.0
		ro["climbed"] = climbed
		ro["climb_s"] = snappedf(t, 0.1)
		ro["stuck_on_the_way"] = stuck > 1.0
		ro["on_top"] = p.is_on_floor() and p.position.y >= rope["top"] - _m(0.6)
		ro["damage"] = p.inventory.max_health - p.inventory.health
		await tree.create_timer(0.5).timeout
		await _film(false, "world_rope")
		_aim_at(Vector3(rope["x"], rope["top"] - _m(0.1), rope["z"]))
		p._use(true)
		ro["taken_back"] = main.toys.ropes.is_empty()
	ro["ok"] = ro["hung"] and ro.get("on_top", false) and ro.get("taken_back", false)
	w["rope"] = ro
	print("toys world rope " + JSON.stringify(ro))
	# ---- the hook: from the foot of the drop, at the face just under the rim ----
	_hold("grappling_hook")
	_heal()
	var foot := edge + dir * _m(6.0)
	foot.y = _terrain_y(int(foot.x), int(foot.z), int(edge.y))
	await _feet(foot + Vector3(0, 0.05, 0))
	_aim_at(edge + dir * _m(0.2) - Vector3(0, _m(1.5), 0))
	p._use(true)
	_film(true, "world_hook")
	var hk := {}
	t = 0.0
	while main.toys.hook == "flying" and t < 1.0:
		await tree.physics_frame
		t += get_physics_process_delta_time()
	hk["caught"] = main.toys.hook == "caught"
	hk["on"] = GameMaterials.name_of(main.get_world_voxel(main.toys._anchor_voxel)) if hk["caught"] else ""
	hk["line_m"] = snappedf(Settings.v2m(main.toys._line), 0.1)
	p.autopilot_hold = true
	await tree.create_timer(1.0).timeout
	p.autopilot_hold = false
	hk["lifted_m"] = snappedf(Settings.v2m(p.position.y - foot.y), 0.1)
	await tree.create_timer(3.0).timeout
	main.toys._let_go_with_swing()
	await tree.create_timer(2.5).timeout
	hk["damage"] = p.inventory.max_health - p.inventory.health
	await _film(false, "world_hook")
	hk["ok"] = hk["caught"] and hk["lifted_m"] > 1.0
	w["hook"] = hk
	print("toys world hook " + JSON.stringify(hk))
	w["ok"] = g["ok"] and ro["ok"] and hk["ok"]
