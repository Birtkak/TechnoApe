class_name FluidLagTest
extends RefCounted

## Debug (fluid_lag=lake|river|bomb|pour, ADR 0137): how long a fluid change takes to show and what it costs a frame.
##   lake / river: a 1.5 m hole dug into the bank just under the surface of the nearest lake / river (it pours in)
##   bomb:         a 1.5 m water ball dropped into a 2.5 m pit on the ground under the player
##   pour:         a 2 m water ball let go at the top of a 4 m cave room carved 6 m under the ground
##   fly:          spectator flight at 15 m/s, 4 m over the nearest lake (the window walks over still water: what wakes)
## From the event frame on it records every frame: frame ms, the bubble's paste ms, the sim tick (ms, moved cells) and
## the mesh state of the 8 m mesh blocks around the spot (godot_voxel: not up to date = a paste waits for its remesh).
## One JSON line "debug fluid_lag {...}": frame p99 / max, tick ms, pastes, remeshes of the watched blocks, latency
## (ms and sim ticks) from the event to the first remesh applied and to rest (sim quiet, queue empty, meshes current).
## fluid_lag_sheet=1 keeps a contact sheet of the first 3 s (every 100 ms; fly: the lake under the parked camera while it
## loads, every 0.5 s) as fluid_lag_<case>.png in the out dir.

const WATCH_S := 25.0
const UP_TO_DATE := 1   # VoxelLodTerrainUpdateData::MESH_UP_TO_DATE

var main: Node
var r := {}


func run(p_main: Node, which: String) -> void:
	main = p_main
	var tree: SceneTree = main.get_tree()
	r["case"] = which
	await tree.create_timer(1.0).timeout
	main.player.autopilot_action = "look"
	main.player.autopilot_speed_m = 0.0
	main.chat.send("/time set noon")
	main.chat.send("/gamemode creative")
	main.player.flying = true
	var me := Vector3i(main.player.position.floor())
	var from := String(DebugCapture.fluid_lag_from).split(",")   # fluid_lag_from=x,z: search from there (voxels)
	if from.size() == 2:
		me = Vector3i(int(from[0]), me.y, int(from[1]))
	var bubble: SimBubble = main.bubble
	var at := Vector3.ZERO
	var event := Callable()
	if which == "lake" or which == "river" or which == "fly":
		var near: Vector3i = main.world.generator.gen.find_nearest("lake" if which == "fly" else which, me.x, me.z, int(Settings.m2v(1500.0)))
		if near.y < 0 and which != "river":
			near = main.world.generator.gen.find_nearest("ocean", me.x, me.z, int(Settings.m2v(3000.0)))
		if near.y < 0:
			_done("no %s near the player" % which)
			return
		await _go(tree, Vector3(near.x, near.y + 40, near.z))
		var bank := _bank(Vector3i(near.x, near.y + 24, near.z))
		if bank.is_empty():
			_done("no water with a bank near " + str(near))
			return
		var top: int = bank["top"]
		var c: Vector3i = bank["bank"] + bank["dir"] * 8   # 1 m into the bank, 0.5 m under the surface
		at = Vector3(c.x, top - 4, c.z) + Vector3(0.5, 0.5, 0.5)
		r["surface_y"] = top
		event = func() -> void: bubble.blast(at, Settings.m2v(1.5))
		if which == "fly":
			at = Vector3(near.x, top + Settings.m2v(4.0), near.z)
			main.chat.send("/gamemode spectator")
			main.chat.send("/tp %d %d %d" % [at.x, at.y, at.z])
			event = func() -> void:
				main.chat.send("/cam off")
				main.player.autopilot_action = ""
				main.player.autopilot_speed_m = 15.0
	else:
		var g := _ground(me)
		for i in 40:   # the spawn's column may still be streaming in
			if g >= 0:
				break
			await tree.create_timer(0.5).timeout
			g = _ground(me)
		if g < 0:
			_done("no ground under the player")
			return
		var base := Vector3(me.x, g, me.z) + Vector3(0.5, 0.5, 0.5)
		if which == "bomb":
			bubble.blast(base, Settings.m2v(2.5))
			at = base + Vector3(0, Settings.m2v(1.5) + 2.0, 0)
			event = func() -> void: bubble.blast(at, Settings.m2v(1.5), GameMaterials.WATER)
		else:
			var room := base - Vector3(0, Settings.m2v(6.0), 0)
			bubble.blast(room, Settings.m2v(4.0))
			at = room + Vector3(0, Settings.m2v(1.5), 0)
			event = func() -> void: bubble.blast(at, Settings.m2v(2.0), GameMaterials.WATER)
	r["at"] = Vector3i(at.floor())
	# a camera parked on the spot (the sheet), then wait for the set-up's own pastes to land
	var cam := at + Vector3(Settings.m2v(5.0), Settings.m2v(4.0), Settings.m2v(5.0))
	main.chat.send("/cam %d %d %d %d %d %d" % [cam.x, cam.y, cam.z, at.x, at.y, at.z])
	var blocks := _blocks(Vector3i(at.floor()), int(Settings.m2v(6.0)))
	var sheet: FrameSheet = FrameSheet.new(256, 144, 6) if DebugCapture.fluid_lag_sheet else null
	var wait0 := Time.get_ticks_msec()
	while Time.get_ticks_msec() - wait0 < 15000:
		await tree.create_timer(0.5).timeout
		if sheet and which == "fly":   # fly: the parked camera over the lake while it loads (generated ice melting or not)
			sheet.add(main.get_viewport())
		if bubble.stats()["push_queue"] == 0 and _stale(blocks) == 0 and bubble.last_moved < 8 and Time.get_ticks_msec() - wait0 > 3000:
			break
	r["setup_wait_ms"] = Time.get_ticks_msec() - wait0
	r["outside_c"] = snappedf(bubble.ambient_c, 0.1)
	r["moving_water_at_event"] = bubble.sim.count_material(GameMaterials.WATER)
	blocks = blocks.filter(func(b: Vector3i) -> bool: return _stale([b]) == 0)   # meshed and current: empty blocks never are
	var next_shot := 0.0
	# the event and the frames after it
	var ticks0 := bubble.ticks_total
	var woken0 := int(bubble.stats()["still_woken"])
	var active_max := 0
	var pushed0 := bubble.chunks_pushed
	var t0 := Time.get_ticks_usec()
	event.call()
	var frames: Array[float] = []
	var spikes: Array = []
	var push_ms_max := 0.0
	var tick_ms_max := 0.0
	var tick_ms_sum := 0.0
	var ticks_seen := 0
	var moved_max := 0
	var queue_max := 0
	var remeshes := 0
	var was_stale := {}   # block -> ms it went stale
	var waits: Array[float] = []   # stale -> remeshed, per episode
	var timeline: Array = []      # every 0.5 s: [s, moved, pastes since the last entry, stale blocks, queue]
	var next_line := 0.0
	var pushed_line := pushed0
	var last_paste_ms := 0.0
	var pushed_prev := pushed0
	var first_mesh_ms := -1.0
	var first_mesh_ticks := -1
	var quiet := 0
	var sim_rest_ms := -1.0
	var sim_rest_ticks := -1
	var rest_ms := -1.0
	var last_tick := ticks0
	var last_frame := Time.get_ticks_usec()
	while (Time.get_ticks_usec() - t0) / 1e6 < WATCH_S:
		await tree.process_frame
		var now := Time.get_ticks_usec()
		frames.append((now - last_frame) / 1000.0)
		if frames[-1] > 16.7 and spikes.size() < 12:   # where the time went: the bubble's split, the engine, godot_voxel
			var sp := {"t": snappedf((now - t0) / 1e6, 0.01), "ms": snappedf(frames[-1], 0.1), "bubble": bubble.last_frame_cost(),
				"process_ms": snappedf(Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0, 0.1),
				"physics_ms": snappedf(Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS) * 1000.0, 0.1), "tick_ms": bubble.last_tick_ms}
			var ts: Dictionary = bubble.last_slow   # read on this frame's start by the bubble (the counters reset on read)
			for k: String in ts:
				if k.begins_with("ta_") and k != "ta_gen_run_sum" and k != "ta_gen_run_n" and ts[k] is int and int(ts[k]) > 2000:
					sp[k] = ts[k]
			sp["slow_frame_ms"] = ts.get("frame_ms", 0.0)
			sp["slowest"] = ts.get("ta_slowest_task", "")
			sp["proc_phase_ms"] = snappedf(DebugCapture._process_us / 1000.0, 0.1)   # the last frame's _process phase and its nodes
			sp["physics_phase_ms"] = snappedf(DebugCapture._physics_us / 1000.0, 0.1)
			var pc: Dictionary = DebugCapture._proc_prev
			for k: String in pc:
				if float(pc[k]) > 1.0:
					sp["p_" + k] = snappedf(float(pc[k]), 0.1)
			spikes.append(sp)
		last_frame = now
		var ms := (now - t0) / 1000.0
		push_ms_max = maxf(push_ms_max, bubble.push_ms)
		var st := bubble.stats()
		queue_max = maxi(queue_max, int(st["push_queue"]))
		active_max = maxi(active_max, bubble.active_chunks)
		if bubble.ticks_total != last_tick:   # one or more ticks finished since the last frame
			last_tick = bubble.ticks_total
			ticks_seen += 1
			tick_ms_sum += bubble.last_tick_ms
			tick_ms_max = maxf(tick_ms_max, bubble.last_tick_ms)
			moved_max = maxi(moved_max, bubble.last_moved)
			quiet = quiet + 1 if bubble.last_moved < 8 else 0
			if quiet >= 10 and sim_rest_ms < 0 and bubble.ticks_total - ticks0 > 12:
				sim_rest_ms = ms
				sim_rest_ticks = bubble.ticks_total - ticks0 - 9
		var stale := 0
		for b: Vector3i in blocks:
			var s := int(main.world.terrain.debug_get_mesh_block_info(Vector3(b), 0).get("mesh_state", UP_TO_DATE))
			if s != UP_TO_DATE:
				stale += 1
				if not was_stale.has(b):
					was_stale[b] = ms
			elif was_stale.has(b):
				waits.append(ms - float(was_stale[b]))
				was_stale.erase(b)
				remeshes += 1
				if first_mesh_ms < 0:
					first_mesh_ms = ms
					first_mesh_ticks = bubble.ticks_total - ticks0
		if bubble.chunks_pushed != pushed_prev:
			last_paste_ms = ms
			pushed_prev = bubble.chunks_pushed
		if ms >= next_line:
			timeline.append([snappedf(ms / 1000.0, 0.1), bubble.last_moved, bubble.chunks_pushed - pushed_line, stale, int(st["push_queue"])])
			pushed_line = bubble.chunks_pushed
			next_line += 500.0
		if sheet and ms >= next_shot and sheet.count() < 30:
			sheet.add(main.get_viewport())
			next_shot += 100.0
		if which != "fly" and sim_rest_ms >= 0 and int(st["push_queue"]) == 0 and stale == 0:
			rest_ms = ms
			if sheet == null or sheet.count() >= 30:
				break
	frames.sort()
	r["frames"] = frames.size()
	r["frame_p99_ms"] = snappedf(frames[mini(frames.size() - 1, int(frames.size() * 0.99))], 0.01) if frames.size() > 0 else 0.0
	r["frame_max_ms"] = snappedf(frames[-1], 0.01) if frames.size() > 0 else 0.0
	r["over_16ms"] = frames.filter(func(f: float) -> bool: return f > 16.7).size()
	r["tick_ms_avg"] = snappedf(tick_ms_sum / maxi(ticks_seen, 1), 0.01)
	r["tick_ms_max"] = snappedf(tick_ms_max, 0.01)
	r["moved_max"] = moved_max
	r["push_ms_max"] = snappedf(push_ms_max, 0.01)
	r["still_woken"] = int(bubble.stats()["still_woken"]) - woken0
	r["active_max"] = active_max
	r["ticks"] = bubble.ticks_total - ticks0
	r["pastes"] = bubble.chunks_pushed - pushed0
	r["push_queue_max"] = queue_max
	r["watched_blocks"] = blocks.size()
	r["remeshes"] = remeshes
	r["first_mesh_ms"] = snappedf(first_mesh_ms, 0.1)
	r["first_mesh_ticks"] = first_mesh_ticks
	r["sim_rest_ms"] = snappedf(sim_rest_ms, 0.1)
	r["sim_rest_ticks"] = sim_rest_ticks
	r["rest_ms"] = snappedf(rest_ms, 0.1)
	r["mesh_behind_sim_ms"] = snappedf(rest_ms - sim_rest_ms, 0.1) if rest_ms >= 0 and sim_rest_ms >= 0 else -1.0
	waits.sort()
	r["remesh_wait_ms_med"] = snappedf(waits[waits.size() / 2], 0.1) if waits.size() > 0 else -1.0
	r["remesh_wait_ms_max"] = snappedf(waits[-1], 0.1) if waits.size() > 0 else -1.0
	r["last_paste_ms"] = snappedf(last_paste_ms, 0.1)
	r["timeline"] = timeline
	r["spikes"] = spikes
	if sheet:
		sheet.save(DebugCapture.out_dir + "/fluid_lag_%s.png" % which)
	_done("")


func _go(tree: SceneTree, p: Vector3) -> void:
	main.chat.send("/tp %d %d %d" % [p.x, p.y, p.z])
	await tree.create_timer(8.0).timeout
	var t := Time.get_ticks_msec()
	while not main.bubble.is_loaded() and Time.get_ticks_msec() - t < 15000:
		await tree.create_timer(0.5).timeout


func _water(c: Vector3i) -> bool:
	var m: int = main.world.get_voxel(c)
	return m == GameMaterials.WATER or GameMaterials.name_of(m) == "water_static"


## The water surface nearest `from` (rings outward) and the first bank along an axis from 3 cells inside it.
func _bank(from: Vector3i) -> Dictionary:
	for ring in range(0, 200, 2):
		for i in maxi(ring * 4, 1):
			var a := TAU * i / maxi(ring * 4, 1)
			var c := from + Vector3i(roundi(cos(a) * ring), 0, roundi(sin(a) * ring))
			for dy in range(0, -80, -1):
				var q := c + Vector3i(0, dy, 0)
				if _water(q):
					for d: Vector3i in [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]:
						for k in range(1, 120):
							var b: Vector3i = q + d * k
							if not _water(b) and GameMaterials.blocks_bodies(main.world.get_voxel(b)) \
									and GameMaterials.blocks_bodies(main.world.get_voxel(b - Vector3i(0, 4, 0))):
								return {"top": q.y, "bank": b, "dir": d}
							if not _water(b):
								break
					break
				if GameMaterials.blocks_bodies(main.world.get_voxel(q)):
					break
	return {}


func _ground(me: Vector3i) -> int:
	# no further down than 5 m under the player: while the spawn's column streams in, its surface reads as air and the
	# scan found a cave 36 m down, outside the sim window, so the bomb fell on nothing
	for y in range(me.y + 80, me.y - int(Settings.m2v(5.0)), -1):
		if GameMaterials.blocks_bodies(main.world.get_voxel(Vector3i(me.x, y, me.z))):
			return y
	return -1


func _blocks(c: Vector3i, rad: int) -> Array[Vector3i]:
	var size: int = main.world.terrain.mesh_block_size
	var out: Array[Vector3i] = []
	var lo := Vector3i((Vector3(c - Vector3i.ONE * rad) / size).floor())
	var hi := Vector3i((Vector3(c + Vector3i.ONE * rad) / size).floor())
	for x in range(lo.x, hi.x + 1):
		for y in range(lo.y, hi.y + 1):
			for z in range(lo.z, hi.z + 1):
				out.append(Vector3i(x, y, z))
	return out


func _stale(blocks: Array[Vector3i]) -> int:
	var n := 0
	for b in blocks:
		if int(main.world.terrain.debug_get_mesh_block_info(Vector3(b), 0).get("mesh_state", UP_TO_DATE)) != UP_TO_DATE:
			n += 1
	return n


func _done(why: String) -> void:
	r["ok"] = why == ""
	if why != "":
		r["why"] = why
	print("debug fluid_lag ", JSON.stringify(r))
	main.get_tree().quit()
