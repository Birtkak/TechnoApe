class_name SimBubble
extends Node

## The Noita-style simulation bubble: a sliding window of chunks around the
## player, owned by the native SimWorld. Chunks entering the window are copied
## from the terrain; the window ticks at Settings.sim_tick_hz; chunks the sim
## changed are pasted back into the terrain. Everything outside is frozen.
##
## Local chunk (0..N) <-> world chunk = local + origin.

## After every tick: the world chunks it changed (StainMap queues them and reads
## them on a later frame while is_idle(); keep handlers cheap, the next tick waits).
signal chunks_changed(world_chunks: Array)

var world: World
var sim: SimWorld
var origin := Vector3i.ZERO          # world chunk coords of local chunk (0,0,0)
var size_chunks := Vector3i.ONE
var _tick_accum := 0.0
var _pending_loads: Array[Vector3i] = []   # local chunks waiting for terrain data
var _ready := false

# stats for the HUD / capture
var last_tick_ms := 0.0
var ticks_total := 0
var chunks_pushed := 0
var chunks_loaded := 0
var bad_loads := 0
var active_chunks := 0
var push_ms := 0.0
var shifts := 0
var _tick_running := false
# Bridge to the brick world (ADR 0012 step 3): when the owner of a BrickStore sets these,
# every loaded chunk and every tick's changed cells are written into the store (one bubble
# cell = one brick, the 0.5 m game), so the ray-cast near field shows what the simulation
# does. NearField gives it a wrapped store addressed by world bricks (store_origin_voxel 0).
var store: BrickStore = null
var store_origin_voxel := Vector3i.ZERO   # world voxel of the store's brick (0,0,0); the owner updates it when the store window slides
var store_writes := 0
var _push_queue := {}          # world chunk -> true, insertion order = oldest first (ADR 0010)
var hold_pushes := true        # no pastes into the terrain until the first streaming burst is done (ADR 0020): an edit
                               # cascades to every LOD, and parents that are not loaded yet log "destination block not found"
var push_queue_max := 0
var _last_push := {}           # world chunk -> msec of its last paste (Settings.sim_push_interval_ms)
var _fire_test := -1           # debug: fire_test=N ignites burnables within N voxels once loaded
var _fire_test_at := -1.0
var fire_ignited := 0
var _keep := PackedInt32Array()   # materials blast() never removes (stations, bedrock), filled on first use
var blasts := 0
var blast_voxels := 0
var blast_ms_max := 0.0
var _fire_center := Vector3i.ZERO   # player voxel (world), updated by follow()
# per-frame cost breakdown (main-thread ms), worst frames kept for the JSON
var _cost := {}
var _worst: Array = []
var _last_cost := {}
const WORST_KEEP := 5
var _slow: Array = []          # whole frames over SLOW_MS: where did the time go (debug JSON)
const SLOW_MS := 6.0


func _frame_cost() -> Dictionary:
	var f := Engine.get_process_frames()
	if _cost.get("frame", -1) != f:
		_commit_cost()
		_cost = {"frame": f, "shift_ms": 0.0, "flush_ms": 0.0, "load_ms": 0.0, "loads": 0, "tick_ms": 0.0, "push_ms": 0.0, "edit_ms": 0.0, "pushed": 0}
	return _cost


## The bubble's cost breakdown of the previous frame (DebugCapture spikes), {} if it did nothing.
func last_frame_cost() -> Dictionary:
	var f := Engine.get_process_frames()
	if _cost.get("frame", -1) == f - 1:
		return _cost
	return _last_cost if _last_cost.get("frame", -1) == f - 1 else {}


func _commit_cost() -> void:
	if _cost.is_empty():
		return
	_last_cost = _cost
	var total: float = _cost["shift_ms"] + _cost["flush_ms"] + _cost["load_ms"] + _cost["tick_ms"] + _cost["push_ms"] + _cost["edit_ms"]
	_cost["total_ms"] = snappedf(total, 0.01)
	for k in ["shift_ms", "flush_ms", "load_ms", "tick_ms", "push_ms", "edit_ms"]:
		_cost[k] = snappedf(_cost[k], 0.01)
	_worst.append(_cost)
	_worst.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a["total_ms"] > b["total_ms"])
	if _worst.size() > WORST_KEEP:
		_worst.resize(WORST_KEEP)


func setup(p_world: World, center_world: Vector3) -> void:
	world = p_world
	var r := Settings.sim_distance_chunks()
	size_chunks = Vector3i(2 * r + 1, 2 * r + 1, 2 * r + 1)
	sim = SimWorld.new()
	sim.configure(size_chunks, Settings.sim_threads, Settings.world_seed)
	sim.set_materials(GameMaterials.defs())
	sim.set_reactions(GameMaterials.reactions())
	# liquids hold 1..8 eighths and spread into thin layers; the terrain draws the level (fluid models)
	sim.set_liquid_fullness(true)
	# Chunks stream in over several frames; until loaded they must act as walls,
	# not air, or water next to them flows into nothing (holes in lakes).
	sim.mark_all_loaded(false)
	origin = _center_chunk(center_world) - Vector3i(r, r, r)
	for z in size_chunks.z:
		for y in size_chunks.y:
			for x in size_chunks.x:
				_pending_loads.append(Vector3i(x, y, z))
	_ready = true
	for a in OS.get_cmdline_user_args():
		if a.begins_with("fire_test="):
			_fire_test = int(a.get_slice("=", 1))


func _center_chunk(p: Vector3) -> Vector3i:
	return Vector3i((p / Settings.CHUNK).floor())


func _process(delta: float) -> void:
	if not _ready:
		return
	if delta * 1000.0 > SLOW_MS and Engine.get_process_frames() > 120:
		_note_slow_frame(delta)
	# v6 (ADR 0012 sim chapter): the tick runs on a background thread. The main thread
	# collects a finished tick, pastes and loads chunks while the simulation is idle,
	# and starts the next tick when one is due; the tick overlaps the rest of the frame.
	_tick_accum += delta
	if _tick_running and sim.is_tick_done():
		var r: Dictionary = sim.finish_tick()
		last_tick_ms = r["ms_per_tick"]
		active_chunks = int(r["active_chunks_per_tick"])
		ticks_total += 1
		_tick_running = false
		_collect_dirty()
	if _tick_running:
		return
	_load_pending()
	if not _push_queue.is_empty() and not hold_pushes:
		var t1 := Time.get_ticks_usec()
		_drain_push(Settings.sim_push_budget_ms)
		push_ms = (Time.get_ticks_usec() - t1) / 1000.0
		_frame_cost()["push_ms"] += push_ms
	var period := 1.0 / Settings.sim_tick_hz
	if _tick_accum >= period:
		_tick_accum = minf(_tick_accum - period, period * 2.0)   # never build up more than two ticks of debt
		var t0 := Time.get_ticks_usec()
		_tick_running = sim.start_tick()
		_frame_cost()["tick_ms"] += (Time.get_ticks_usec() - t0) / 1000.0   # main-thread cost of starting it
	if _fire_test > 0:
		_run_fire_test()


## Keep the window centred on the player; shift by whole chunks.
func follow(center_world: Vector3) -> void:
	if not _ready:
		return
	_fire_center = Vector3i(center_world.floor())
	var r := (size_chunks.x - 1) / 2
	var want := _center_chunk(center_world) - Vector3i(r, r, r)
	var d := want - origin
	if d == Vector3i.ZERO:
		return
	# Shift at most one chunk per axis per call to keep spikes small.
	d = d.clamp(Vector3i(-1, -1, -1), Vector3i(1, 1, 1))
	var t0 := Time.get_ticks_usec()
	_flush_leaving(d)  # chunks sliding out of the window must reach the terrain first
	var t1 := Time.get_ticks_usec()
	var vacated: Array = sim.shift(d)
	var cost := _frame_cost()
	cost["flush_ms"] += (t1 - t0) / 1000.0
	cost["shift_ms"] += (Time.get_ticks_usec() - t1) / 1000.0
	shifts += 1
	origin += d
	for c in vacated:
		_pending_loads.append(c)


## Loads vacated / new chunks from the terrain within a main-thread time
## budget (Settings.sim_load_budget_ms, at least MIN_LOADS per frame), so a
## shift's 49 chunks spread over a few frames instead of one ~0.7 ms frame.
## Chunks still waiting act as walls in the simulation.
const MIN_LOADS := 4


func _load_pending() -> void:
	var t0 := Time.get_ticks_usec()
	var budget_us := int(Settings.sim_load_budget_ms * 1000.0)
	var still: Array[Vector3i] = []
	var n := 0
	for lc in _pending_loads:
		var wc: Vector3i = lc + origin
		var over := n >= MIN_LOADS and Time.get_ticks_usec() - t0 >= budget_us
		if over or not world.is_area_loaded(wc):
			still.append(lc)
			continue
		var bytes: PackedByteArray = world.read_chunk(wc)
		if bytes.size() != 8192:
			bad_loads += 1
			if bad_loads <= 3:
				push_warning("SimBubble: chunk %s read %d bytes (expected 8192)" % [wc, bytes.size()])
		sim.set_chunk_bytes(lc, bytes, 2)
		if store != null:   # the near field renders from the store (ADR 0012 step 3); the sim decoded the fluid levels
			sim.write_chunk_to_store(store, lc, origin * Settings.CHUNK - store_origin_voxel)
		chunks_loaded += 1
		n += 1
	_pending_loads = still
	if n > 0:
		var c := _frame_cost()
		c["load_ms"] += (Time.get_ticks_usec() - t0) / 1000.0
		c["loads"] += n


## Records a slow frame with Godot's own monitors (previous frame) next to the
## bubble's costs, so a spike can be attributed without a profiler.
func _note_slow_frame(delta: float) -> void:
	var prev: Dictionary = _cost if _cost.get("frame", -1) == Engine.get_process_frames() - 1 else {}
	var t: Dictionary = world.terrain.get_statistics()
	_slow.append({"frame_ms": snappedf(delta * 1000.0, 0.01),
		"process_ms": snappedf(Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0, 0.01),
		"physics_ms": snappedf(Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS) * 1000.0, 0.01),
		"nav_ms": snappedf(Performance.get_monitor(Performance.TIME_NAVIGATION_PROCESS) * 1000.0, 0.01),
		"draw_calls": Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		"objects": Performance.get_monitor(Performance.RENDER_TOTAL_OBJECTS_IN_FRAME),
		"vram_mb": snappedf(Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0, 1),
		"bubble_ms": prev.get("total_ms", 0.0), "pushed": prev.get("pushed", 0),
		"terrain_update_ms": t.get("time_update_task", 0), "terrain_mesh_req": t.get("time_mesh_requests", 0)})
	_slow.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a["frame_ms"] > b["frame_ms"])
	if _slow.size() > 8:
		_slow.resize(8)


## Moves the sim's dirty chunks into the push queue (world coords, so they
## survive window shifts).
func _collect_dirty() -> void:
	if store != null:
		store_writes += sim.write_dirty_to_store(store, origin * Settings.CHUNK - store_origin_voxel, 8)
	var changed: Array = []
	for lc in sim.get_dirty_chunks():
		if not _pending_loads.has(lc):   # never paste a chunk before it was loaded
			_push_queue[lc + origin] = true
			changed.append(lc + origin)
	if not changed.is_empty():
		chunks_changed.emit(changed)
	sim.clear_dirty()
	push_queue_max = maxi(push_queue_max, _push_queue.size())


## Pastes queued chunks into the terrain, oldest first, until `budget_ms` of
## main-thread time is used (at least one per call). Each paste costs ~0.1 ms
## plus a remesh; a big fire dirties 30+ chunks per tick, which unbudgeted
## turned every sim tick into a multi-millisecond frame (ADR 0010).
func _drain_push(budget_ms: float) -> void:
	var t0 := Time.get_ticks_usec()
	var budget_us := int(budget_ms * 1000.0)
	var now := Time.get_ticks_msec()
	var interval := int(Settings.sim_push_interval_ms)
	var done: Array[Vector3i] = []
	for wc in _push_queue:
		if not done.is_empty() and Time.get_ticks_usec() - t0 >= budget_us:
			break
		if interval > 0 and now - int(_last_push.get(wc, -100000)) < interval:
			continue   # pasted moments ago: let more changes pile up, one remesh instead of several
		_push_one(wc)
		_last_push[wc] = now
		done.append(wc)
	for wc in done:
		_push_queue.erase(wc)


func _push_one(wc: Vector3i) -> void:
	var lc: Vector3i = wc - origin
	if lc.x < 0 or lc.y < 0 or lc.z < 0 or lc.x >= size_chunks.x or lc.y >= size_chunks.y or lc.z >= size_chunks.z:
		return   # left the window (flushed on shift already)
	world.write_chunk(wc, sim.get_chunk_bytes(lc, 2))
	chunks_pushed += 1
	_frame_cost()["pushed"] += 1


## Before shifting by d: push everything queued that is about to leave the
## window; the rest stays queued and is pushed from its new place.
func _flush_leaving(d: Vector3i) -> void:
	_collect_dirty()
	var leaving: Array[Vector3i] = []
	for wc in _push_queue:
		var nl: Vector3i = wc - origin - d
		if nl.x < 0 or nl.y < 0 or nl.z < 0 or nl.x >= size_chunks.x or nl.y >= size_chunks.y or nl.z >= size_chunks.z:
			leaving.append(wc)
	for wc in leaving:
		_push_one(wc)
		_push_queue.erase(wc)


## Pushes everything now (tests, saving, quitting).
func flush_all() -> void:
	_collect_dirty()
	_drain_push(1e9)


## Debug (fire_test=N on the command line): once the chunks around the player
## have loaded (6 s after start), ignite every burnable voxel within N voxels
## of the player.
func _run_fire_test() -> void:
	if _fire_test_at < 0.0:
		_fire_test_at = Time.get_ticks_msec() / 1000.0 + 6.0
		return
	if Time.get_ticks_msec() / 1000.0 < _fire_test_at:
		return
	var c: Vector3i = _fire_center - origin * Settings.CHUNK   # player voxel in window coords
	var r := Vector3i.ONE * _fire_test
	fire_ignited += sim.ignite_box(c - r, c + r + Vector3i.ONE)
	print("fire_test: ignited %d voxels around %s" % [fire_ignited, _fire_center])
	_fire_test = 0


## Mass edit (explosion, black hole, big dig; ADR 0012 requires these never to choke a
## frame): every voxel whose centre lies within `radius` voxels of `center` (world
## voxel coords) becomes `material`, in one native call. Stations and bedrock stay
## (block entities must not vanish under their data). Only the window is edited:
## outside it the world is frozen and not simulated; chunks still loading are skipped.
## The changed chunks reach the terrain through the budgeted push queue, and the
## kernel loosens static ground around the hole (ADR 0013), so craters collapse.
## Returns the number of voxels changed.
func blast(center: Vector3, radius: float, material := GameMaterials.AIR) -> int:
	if not _ready:
		return 0
	if _keep.is_empty():
		var defs := GameMaterials.defs()
		for i in defs.size():
			var d: Dictionary = defs[i]
			if d.has("interact") or d.has("entity") or float(d.get("hp", 0)) >= 1e6:
				_keep.append(i + 1)
	var t0 := Time.get_ticks_usec()
	var n := sim.fill_sphere(center - Vector3(origin * Settings.CHUNK), radius, material, _keep)
	var ms := (Time.get_ticks_usec() - t0) / 1000.0
	_frame_cost()["edit_ms"] = _frame_cost().get("edit_ms", 0.0) + ms
	blasts += 1
	blast_voxels += n
	blast_ms_max = maxf(blast_ms_max, ms)
	return n


## Player edits go through the sim when inside the bubble (so the material
## simulation reacts immediately), otherwise straight to the terrain.
func set_voxel(wp: Vector3i, material: int) -> bool:
	var lc := Vector3i((Vector3(wp) / Settings.CHUNK).floor()) - origin
	if lc.x < 0 or lc.y < 0 or lc.z < 0 or lc.x >= size_chunks.x or lc.y >= size_chunks.y or lc.z >= size_chunks.z:
		return false
	if _pending_loads.has(lc):
		return false
	sim.set_voxel(wp - origin * Settings.CHUNK, material)
	return true


## The bubble's current material at a world voxel, or -1 when the voxel is
## outside the window or its chunk is still loading. Newer than the terrain:
## edits reach the terrain only on the next push.
func get_voxel(wp: Vector3i) -> int:
	if not _ready:
		return -1
	var lc := Vector3i((Vector3(wp) / Settings.CHUNK).floor()) - origin
	if lc.x < 0 or lc.y < 0 or lc.z < 0 or lc.x >= size_chunks.x or lc.y >= size_chunks.y or lc.z >= size_chunks.z:
		return -1
	if _pending_loads.has(lc):
		return -1
	return sim.get_voxel(wp - origin * Settings.CHUNK)


## Debug: compare every loaded bubble chunk with what the generator says it
## should be. In a settled, unedited world they must match; mismatches mean
## chunks were loaded while the terrain was still streaming (air where there
## should be ground) or the simulation moved something.
func verify_against_generator(gen: WorldGen) -> Dictionary:
	if not _ready:
		return {}
	var mismatched := 0
	var air_where_solid := 0
	var solid_where_air := 0
	var checked := 0
	var examples := []
	for z in size_chunks.z:
		for y in size_chunks.y:
			for x in size_chunks.x:
				var lc := Vector3i(x, y, z)
				if _pending_loads.has(lc):
					continue
				var wc := lc + origin
				var expect: PackedByteArray = gen.generate_block(wc * Settings.CHUNK, Vector3i(16, 16, 16), 0, 2)
				var got: PackedByteArray = sim.get_chunk_bytes(lc, 2)
				checked += 1
				if expect.is_empty():
					expect = PackedByteArray()
					expect.resize(8192)
				if expect == got:
					continue
				mismatched += 1
				var aws := 0
				var swa := 0
				for i in 4096:
					var e := expect[i * 2] | (expect[i * 2 + 1] << 8)
					var g := got[i * 2] | (got[i * 2 + 1] << 8)
					if e != 0 and g == 0:
						aws += 1
					elif e == 0 and g != 0:
						swa += 1
				air_where_solid += aws
				solid_where_air += swa
				if examples.size() < 4:
					# which material pairs differ (expected -> got), top 4
					var pairs := {}
					for i in 4096:
						var e := expect[i * 2] | (expect[i * 2 + 1] << 8)
						var g := got[i * 2] | (got[i * 2 + 1] << 8)
						if e != g:
							var key := "%s->%s" % [GameMaterials.name_of(e), GameMaterials.name_of(g)]
							pairs[key] = pairs.get(key, 0) + 1
					# what the terrain itself holds right now for this chunk, compared with the generator
					var terrain_now: PackedByteArray = world.read_chunk(wc)
					var terrain_diff := 0
					if terrain_now.size() == expect.size():
						for i in 4096:
							if terrain_now[i * 2] != expect[i * 2] or terrain_now[i * 2 + 1] != expect[i * 2 + 1]:
								terrain_diff += 1
					examples.append({"world_chunk": wc, "air_where_solid": aws, "solid_where_air": swa,
						"terrain_loaded_now": world.is_area_loaded(wc), "pairs": pairs,
						"terrain_bytes": terrain_now.size(), "terrain_vs_generator_diff": terrain_diff,
						"sim_chunk": sim.chunk_info(lc)})
	return {"checked": checked, "mismatched": mismatched, "air_where_solid": air_where_solid, "solid_where_air": solid_where_air, "examples": examples}


## True while no tick runs in the background: the simulation may be read without waiting.
func is_idle() -> bool:
	return _ready and not _tick_running


## True when the local chunk `lc` holds terrain data (not waiting to load).
func is_chunk_loaded(lc: Vector3i) -> bool:
	return _ready and not _pending_loads.has(lc)


## True once every chunk of the window holds terrain data (DebugCapture starts its budget clock here).
func is_loaded() -> bool:
	return _ready and _pending_loads.is_empty()


func stats() -> Dictionary:
	if not _ready:
		return {}
	return {
		"origin_chunk": origin, "size_chunks": size_chunks, "pending_loads": _pending_loads.size(),
		"tick_ms": snappedf(last_tick_ms, 0.01), "ticks": ticks_total, "active_chunks": active_chunks,
		"chunks_pushed": chunks_pushed, "chunks_loaded": chunks_loaded, "bad_loads": bad_loads, "push_ms": snappedf(push_ms, 0.01),
		"shifts": shifts, "worst_frames": _worst, "slow_frames": _slow, "push_queue": _push_queue.size(), "push_queue_max": push_queue_max,
		"burning": sim.count_burning() if _fire_test == 0 else -1, "fire_ignited": fire_ignited,
		"store_writes": store_writes,
		"blasts": blasts, "blast_voxels": blast_voxels, "blast_ms_max": snappedf(blast_ms_max, 0.01),
	}
