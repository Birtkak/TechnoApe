class_name TpTest
extends Node

## Debug (tp_test=1, bug report 2026-10-25 "make teleporting around more smooth"): a scripted burst of
## teleports through the real commands (/tp hops of 15, 60 and 95 m, far jumps of 500 m and 3 km, /spawn,
## /goto, /kill), in survival on foot. Per jump it measures how long a cover stays up, the frame times
## under it and in the 4 s after it (what the player sees), whether the ground was there (fall, damage,
## below the terrain surface) and whether the near terrain was meshed at full detail when the view came
## back. A filmstrip row per jump goes to <out>/tp_strip.png (before, the first frames, the lift and the
## 2.5 s after it); one JSON line "debug tp {...}" sums it up.

const AFTER_S := 4.0        # frames measured after the view comes back
const WAIT_CAP_S := 60.0    # a cover that stays longer counts as a hang
const SHOTS_FROM_TP := [0.0, 0.1, 0.3, 0.6]   # seconds after the jump (plus one shot just before it)
const SHOTS_FROM_LIFT := [0.0, 0.15, 0.3, 0.6, 1.2, 2.5]

var main: Node
var r := {"jumps": []}
var _sheet: FrameSheet
var _last_us := 0
var _skip := false          # the frame after a capture is a capture stall: not counted
var _rec := {}              # the jump being measured
var _frames: PackedFloat64Array = PackedFloat64Array()
var _phase := ""            # "", "cover", "after"
var _t := 0.0               # seconds since the jump
var _lift_t := -1.0
var _shots_tp: Array = []
var _shots_lift: Array = []
var _ground_s := -1.0
var _past_cut := 0
var _landed_m := 0.0        # the hardest landing since the jump (Player.landed)


func run(p_main: Node) -> void:
	main = p_main
	name = "TpTest"
	process_priority = -1000
	main.add_child(self)
	_add_probes()
	main.player.landed.connect(func(_at: Vector3i, m: float) -> void: _landed_m = maxf(_landed_m, m))
	_sheet = FrameSheet.new(256, 144, 1 + SHOTS_FROM_TP.size() + SHOTS_FROM_LIFT.size())
	var tree := get_tree()
	await tree.create_timer(1.0).timeout
	main.player.autopilot_action = "look"
	main.player.autopilot_speed_m = 0.0
	main.chat.send("/time set noon")
	main.chat.send("/gamemode survival")
	main.player.flying = false
	await tree.create_timer(2.0).timeout
	var o: Vector3 = main.player.position
	await _jump("hop_15m", "tp", _ground(o + Vector3(Settings.m2v(15.0), 0, 0)))
	await _jump("hop_60m", "tp", _ground(o + Vector3(Settings.m2v(75.0), 0, 0)))
	await _jump("hop_95m", "tp", _ground(o + Vector3(Settings.m2v(-20.0), 0, 0)))
	await _jump("far_500m", "tp", _ground(o + Vector3(0, 0, Settings.m2v(500.0))))
	await _jump("far_3km", "tp", _ground(o + Vector3(Settings.m2v(3000.0), 0, Settings.m2v(500.0))))
	await _jump("hop_40m_far", "tp", _ground(main.player.position + Vector3(Settings.m2v(40.0), 0, 0)))
	await _jump("spawn", "/spawn")
	await _jump("goto_lake", "/goto lake")
	await _jump("goto_vein", "/goto cassiterite_vein")
	await _jump("kill_respawn", "/kill")
	_report()


func _report() -> void:
	var worst_after := 0.0
	var over := 0
	var fell := 0
	var hangs := 0
	var over_past_cut := 0
	for j in r["jumps"]:
		worst_after = maxf(worst_after, j["after_max_ms"])
		over += int(j["after_over_16_7"])
		over_past_cut += int(j["after_over_16_7_past_cut"])
		fell += 1 if j["below_ground"] or j["damage"] > 0 else 0
		hangs += 1 if j["cover_s"] >= WAIT_CAP_S else 0
	r["after_worst_ms"] = worst_after
	r["after_frames_over_16_7"] = over
	r["after_frames_over_16_7_past_cut"] = over_past_cut
	r["jumps_fallen"] = fell
	r["cover_hangs"] = hangs
	r["ok"] = over_past_cut == 0 and fell == 0 and hangs == 0
	_sheet.save(DebugCapture.out_dir + "/tp_strip.png")
	print("debug tp " + JSON.stringify(r))
	if not DebugCapture._done:
		DebugCapture._finish()   # the run ends with the burst (duration= only caps it)


## Feet a metre above the generator's surface at p.x, p.z.
func _ground(p: Vector3) -> Vector3:
	var h := float(main.world.generator.gen.height_at(int(p.x), int(p.z)))
	return Vector3(floor(p.x) + 0.5, h + Settings.m2v(1.0), floor(p.z) + 0.5)


func _jump(label: String, how: String, to: Vector3 = Vector3.ZERO) -> void:
	var only := ""   # tp_only=hop_60m,far_500m: just these jumps
	for a in OS.get_cmdline_user_args():
		if a.begins_with("tp_only="):
			only = a.substr(8)
	if only != "" and not label in only.split(","):
		return
	var tree := get_tree()
	var from: Vector3 = main.player.position
	_sheet.add(get_viewport())
	_skip = true
	await tree.process_frame
	_landed_m = 0.0
	_ground_s = -1.0
	_past_cut = 0
	if how == "tp":
		main.chat.send("/tp %d %d %d" % [int(to.x), int(ceil(to.y)), int(to.z)])
	else:
		main.chat.send(how)
	var at: Vector3 = main.player.position
	_rec = {"name": label, "dist_m": snappedf(Settings.v2m(Vector2(at.x - from.x, at.z - from.z).length()), 0.1),
		"arrive_y": snappedf(at.y, 0.1), "cover_s": 0.0, "covered": false}
	_frames = PackedFloat64Array()
	_t = 0.0
	_lift_t = -1.0
	_shots_tp = SHOTS_FROM_TP.duplicate()
	_shots_lift = SHOTS_FROM_LIFT.duplicate()
	_phase = "cover"
	var min_y := at.y
	while _phase != "":
		await tree.process_frame
		min_y = minf(min_y, main.player.position.y)
	var p: Vector3 = main.player.position
	var surf := float(main.world.generator.gen.height_at(int(p.x), int(p.z)))
	_rec["fall_m"] = snappedf(Settings.v2m(at.y - min_y), 0.01)
	_rec["landed_m"] = snappedf(_landed_m, 0.01)
	_rec["damage"] = maxi(0, int(floor(_landed_m - Settings.fall_damage_safe_m)))   # what the landing would cost in survival
	_rec["below_ground"] = min_y < surf - Settings.m2v(2.0) and not label.begins_with("goto_vein") and not label.begins_with("goto_cave")
	_rec["end_y_over_surface_m"] = snappedf(Settings.v2m(p.y - surf), 0.01)
	r["jumps"].append(_rec)
	print("tp jump " + JSON.stringify(_rec))
	await tree.create_timer(0.5).timeout


func _covered() -> bool:
	var l = main.get("_loading")
	return l != null and is_instance_valid(l) and not l.is_done()


func _process(delta: float) -> void:
	_stamp("TpTest")
	var now := Time.get_ticks_usec()
	var ms := (now - _last_us) / 1000.0 if _last_us > 0 else delta * 1000.0
	_last_us = now
	if _phase == "":
		_skip = false
		return
	var counted := not _skip
	_skip = false
	_t += delta
	if counted and _t < 0.25 and ms > float(_rec.get("jump_frame_ms", 0.0)):   # the jump's own frames (cover or not)
		_rec["jump_frame_ms"] = snappedf(ms, 0.1)
		_rec["jump_frame_top"] = _top_segments()
	if _ground_s < 0.0 and main.player._ground_ready():
		_ground_s = _t
		_rec["ground_wait_s"] = snappedf(_t, 0.01)   # how long the body hung waiting for its voxels
	if _phase == "cover":
		var lc = main.get("_loading")
		if lc != null and is_instance_valid(lc) and not lc.has_meta("probed"):
			lc.set_meta("probed", true)
			var pl := _Probe.new()
			pl.tag = "LoadingScreen"
			pl.t = self
			lc.add_sibling(pl)
		if _covered():
			_rec["covered"] = true
			if counted and ms > float(_rec.get("cover_max_ms", 0.0)):
				_rec["cover_max_ms"] = snappedf(ms, 0.01)
				_rec["cover_max_top"] = _top_segments()
				_rec["cover_max_at_s"] = snappedf(_t, 0.01)
				_rec["cover_max_physics_ms"] = snappedf(DebugCapture._physics_us / 1000.0, 0.1)
				_rec["cover_max_process_ms"] = snappedf(DebugCapture._process_us / 1000.0, 0.1)
			if _t >= WAIT_CAP_S:
				_lift()
		elif _t > 0.0:
			_lift()
	elif counted:
		_frames.push_back(ms)
		if ms > 16.7 and _t >= 0.25:
			_past_cut += 1
		if ms > 16.7:   # where a slow frame after the lift went (process = scripts + VoxelLodTerrain's main-thread work)
			var sl: Array = _rec.get("after_slow", [])
			var t: Dictionary = VoxelEngine.get_stats().get("tasks", {})
			sl.append({"t": snappedf(_t - _lift_t, 0.01), "ms": snappedf(ms, 0.1), "process_ms": snappedf(DebugCapture._process_us / 1000.0, 0.1),
				"physics_ms": snappedf(DebugCapture._physics_us / 1000.0, 0.1), "mesh_mb": snappedf(DebugCapture._mesh_delta_mb, 0.01),
				"top": _top_segments(), "rcpu": snappedf(RenderingServer.viewport_get_measured_render_time_cpu(get_viewport().get_viewport_rid()) + RenderingServer.get_frame_setup_time_cpu(), 0.1),
				"rgpu": snappedf(RenderingServer.viewport_get_measured_render_time_gpu(get_viewport().get_viewport_rid()), 0.1), "pipelines": DebugCapture._compile_delta, "gen": int(t.get("generation", 0)), "mesh": int(t.get("meshing", 0)), "main": int(t.get("main_thread", 0))})
			sl.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a["ms"] > b["ms"])
			_rec["after_slow"] = sl.slice(0, 4)
		if _t - _lift_t >= AFTER_S:
			_frames.sort()
			_rec["after_max_ms"] = snappedf(_frames[_frames.size() - 1] if _frames.size() else 0.0, 0.01)
			_rec["after_p99_ms"] = snappedf(_frames[int(_frames.size() * 0.99)] if _frames.size() else 0.0, 0.01)
			var over := 0
			for f in _frames:
				over += 1 if f > 16.7 else 0
			_rec["after_over_16_7"] = over
			_rec["after_over_16_7_past_cut"] = _past_cut   # not counting the jump's own first 0.25 s (no cover: the cut itself)
			_rec["after_frames"] = _frames.size()
			_rec["meshed_after"] = _meshed()
			_rec["shown_after"] = snappedf(main.world.near_ready(main.player.position), 0.01)
			_phase = ""
	# filmstrip: shots at fixed times after the jump (under the cover too), then after the lift
	if not _shots_tp.is_empty() and _t >= float(_shots_tp[0]):
		_shots_tp.pop_front()
		_shot()
	elif _lift_t >= 0.0 and _shots_tp.is_empty() and not _shots_lift.is_empty() and _t - _lift_t >= float(_shots_lift[0]):
		_shots_lift.pop_front()
		_shot()


func _lift() -> void:
	_lift_t = _t
	_rec["cover_s"] = snappedf(_t, 0.01) if _rec["covered"] else 0.0
	_rec["meshed_at_lift"] = _meshed()
	_rec["shown_at_lift"] = snappedf(main.world.near_ready(main.player.position), 0.01)
	var t: Dictionary = VoxelEngine.get_stats().get("tasks", {})
	_rec["pending_at_lift"] = int(t.get("generation", 0)) + int(t.get("meshing", 0)) + int(t.get("streaming", 0)) + int(t.get("main_thread", 0))
	_phase = "after"


func _shot() -> void:
	if _sheet.count() >= (1 + SHOTS_FROM_TP.size() + SHOTS_FROM_LIFT.size()) * r["jumps"].size() + 1 + SHOTS_FROM_TP.size() + SHOTS_FROM_LIFT.size():
		return
	_sheet.add(get_viewport())
	_skip = true


## Which rings around the player are meshed: within 16 m and within 32 m (one LOD, ADR 0073).
func _meshed() -> Array:
	var p: Vector3 = main.player.position
	var out := []
	for r_m in [16.0, 32.0]:
		var rr := Settings.m2v(r_m)
		var box := AABB(Vector3(p.x - rr, p.y - rr * 0.25, p.z - rr), Vector3(rr * 2.0, rr * 0.5, rr * 2.0))
		out.append(main.world.terrain.is_area_meshed(box, 0))
	return out


# ---- where a frame's _process time went: a probe node after each child of Main and of World ----

class _Probe extends Node:
	var tag := ""
	var t: TpTest

	func _process(_d: float) -> void:
		t._stamp(tag)


var _stamps: Array = []       # [tag, usec] this frame, in processing order
var _prev_stamps: Array = []  # the frame just finished


func _add_probes() -> void:
	for e in [["DebugCapture", -999999], ["trailing nodes", 999999]]:
		var pe := _Probe.new()
		pe.tag = e[0]
		pe.t = self
		pe.process_priority = e[1]
		main.add_child(pe)
	for parent in [main, main.world]:
		for c in parent.get_children():
			if c is _Probe or c == self:
				continue
			var pr := _Probe.new()
			pr.tag = String(c.name)
			pr.t = self
			c.add_sibling(pr)


func _stamp(tag: String) -> void:
	if tag == "DebugCapture":   # the first probe of a frame
		_prev_stamps = _stamps
		_stamps = [["start", DebugCapture._process_t0]]
	_stamps.append([tag, Time.get_ticks_usec()])


## The three slowest stretches of the last frame's processing: the node before each probe (ms).
func _top_segments() -> Array:
	var segs := []
	for i in range(1, _prev_stamps.size()):
		segs.append([_prev_stamps[i][0], (_prev_stamps[i][1] - _prev_stamps[i - 1][1]) / 1000.0])
	segs.sort_custom(func(a: Array, b: Array) -> bool: return a[1] > b[1])
	return segs.slice(0, 3).map(func(e: Array) -> String: return "%s %.1f" % [e[0], e[1]])
