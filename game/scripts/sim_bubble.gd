class_name SimBubble
extends Node

## The Noita-style simulation bubble (ADR 0033): the native SimWorld ticks the
## world's own 12.5 cm voxels in a wrapped window of Settings.sim_window_chunks
## (64 x 64 x 64 m, ADR 0050) around the player. Moving the window moves an offset, not
## memory (SimWorld.set_wrapped): the chunks that enter are loaded from the
## terrain one chunk column per copy, nearest first, under a main-thread budget;
## until loaded they are walls. Chunks the simulation changed are pasted back
## into the terrain through a budgeted queue (ADR 0010). Outside it the world is frozen, except in pins: small
## windows left where something still moved when the window walked on (ADR 0050).
##
## Local chunk (0..N) <-> world chunk = local + origin; the kernel maps local to
## physical memory itself.

## After every tick: the world chunks it changed (StainMap queues them and reads
## them on a later frame while is_idle(); keep handlers cheap, the next tick waits).
signal chunks_changed(world_chunks: Array)

## Where the player sits in the window (fraction of each axis): low in y,
## because things fall. The window moves once the player strays SLACK chunks.
const CENTER := Vector3(0.5, 0.4, 0.5)
const SLACK := Vector3(2.0, 1.5, 2.0)

var world: World
var sim: SimWorld
var origin := Vector3i.ZERO          # world chunk coords of local chunk (0,0,0)
var size_chunks := Vector3i.ONE
var _tick_accum := 0.0
# Chunks waiting for terrain data, one bitmask per chunk column of the window:
# Vector2i(world chunk x, z) -> bit k set = local chunk y k is unloaded (a wall).
# A window move touches only the columns that change, never every chunk.
var _pending := {}
var _player := Vector3.ZERO
var _ready := false
var pieces: FallingPieces          # rigid voxel pieces (P12), null with rigid_pieces=0
# ADR 0050: what still moves when the window walks away keeps simulating in a pinned window (a small SimBubble that
# never moves, a child of the main one) until it settles. Chunks the main window covers are walls in a pin.
const PIN_CHUNKS := Vector3i(8, 8, 8)   # 16 m cube
const PIN_MAX := 2
const PIN_QUIET_TICKS := 60             # nothing active this long: the pin lets go
const PIN_MAX_S := 90.0
const PIN_PUSH_SHARE := 0.3              # a pin pastes on a third of the main window's budget (one chunk a frame)
var pinned := false                # this bubble is a pin
var owner_bubble: SimBubble        # a pin's main window
var pins: Array[SimBubble] = []
var pins_made := 0
var pin_ms_max := 0.0
var _pin_worst := {}               # the worst frame any pin had (its own cost breakdown)
var _pin_quiet := 0
var _recent := {}                  # world chunk -> ticks_total when the sim last changed it (what still moves)
const RECENT_TICKS := 15
var _pin_age := 0.0

# stats for the HUD / capture
var last_tick_ms := 0.0
var ticks_total := 0
var chunks_pushed := 0
var chunks_loaded := 0
var columns_loaded := 0
var active_chunks := 0
var push_ms := 0.0
var shifts := 0
var scanned_max := 0
var _tick_running := false
var _push_queue := {}          # world chunk -> true, insertion order = oldest first (ADR 0010)
var hold_pushes := true        # no pastes into the terrain until the first streaming burst is done (ADR 0020): an edit
                               # into blocks that are not loaded yet logs "destination block not found"
var push_queue_max := 0
var _last_push := {}           # world chunk -> msec of its last paste (Settings.sim_push_interval_ms)
var _fire_test := -1           # debug: fire_test=N ignites burnables within N voxels once loaded
var _fire_test_at := -1.0
var fire_ignited := 0
var _keep := PackedInt32Array()   # materials blast() never removes (stations, bedrock), filled on first use
var blasts := 0
var blast_voxels := 0
var blast_ms_max := 0.0
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


func setup(p_world: World, center_world: Vector3, pin_of: SimBubble = null) -> void:
	world = p_world
	size_chunks = Settings.sim_window_chunks
	if pin_of:
		pinned = true
		owner_bubble = pin_of
		size_chunks = PIN_CHUNKS
	for a in OS.get_cmdline_user_args():
		if pinned:
			break
		if a.begins_with("sim_window="):   # sim_window=32,16,32 (chunks, powers of two): window size benchmarks
			var v := a.get_slice("=", 1).split(",")
			if v.size() == 3:
				size_chunks = Vector3i(int(v[0]), int(v[1]), int(v[2]))
	sim = SimWorld.new()
	sim.configure(size_chunks, mini(2, Settings.sim_threads) if pinned else Settings.sim_threads, Settings.world_seed)
	if not sim.set_wrapped(true):
		push_error("SimBubble: window chunk counts must be powers of two")
	sim.set_materials(GameMaterials.defs())
	sim.set_reactions(GameMaterials.reactions())
	# liquids hold 1..8 eighths and spread into thin layers; the terrain draws the level (fluid models)
	sim.set_liquid_fullness(true)
	sim.set_cascade(true)   # a column of sand or water falls together, not one 12.5 cm layer per tick
	sim.set_gravity(9.8 / Settings.VOXEL_M / (Settings.sim_tick_hz * Settings.sim_tick_hz))
	sim.set_lod(int(Settings.m2v(Settings.sim_lod_m) / Settings.CHUNK), 2)
	sim.set_cell_budget(Settings.sim_cell_budget)
	# Chunks stream in over several frames; until loaded they must act as walls,
	# not air, or water next to them flows into nothing (holes in lakes).
	sim.mark_all_loaded(false)
	_player = center_world
	origin = _target(center_world)
	if pinned:
		origin = Vector3i((center_world / Settings.CHUNK).floor()) - size_chunks / 2
		# the terrain keeps voxel data only near a viewer (view_distance_m): a data-only viewer keeps it here
		var v := VoxelViewer.new()
		v.name = "Viewer"
		v.requires_visuals = false
		v.requires_collisions = false
		v.view_distance = int(Settings.m2v(Settings.view_distance_m + 8.0))
		v.position = center_world
		add_child(v)
	_queue_box(origin, size_chunks)
	_ready = true
	for a in OS.get_cmdline_user_args():
		if pinned:
			break
		if a.begins_with("fire_test="):
			_fire_test = int(a.get_slice("=", 1))


func _target(p: Vector3) -> Vector3i:
	var c := p / Settings.CHUNK - Vector3(size_chunks) * CENTER
	return Vector3i(roundi(c.x), roundi(c.y), roundi(c.z))


## Queues every chunk column of the window for loading.
func _queue_box(wmin: Vector3i, size: Vector3i) -> void:
	var full := (1 << size.y) - 1
	for z in size.z:
		for x in size.x:
			_pending[Vector2i(wmin.x + x, wmin.z + z)] = full


## True while world chunk `wc` (inside the window) waits for terrain data.
func _is_unloaded(wc: Vector3i) -> bool:
	var ly := wc.y - origin.y
	if ly < 0 or ly >= size_chunks.y:
		return false
	return (int(_pending.get(Vector2i(wc.x, wc.z), 0)) >> ly) & 1 == 1


func _process(delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(delta)
	DebugCapture.note_proc("bubble", _pt0)


func _process_timed(delta: float) -> void:
	if not _ready:
		return
	if delta * 1000.0 > SLOW_MS and Engine.get_process_frames() > 120:
		_note_slow_frame(delta)
	# The tick runs on a background thread (ADR 0012 sim chapter). The main thread
	# collects a finished tick, moves the window, pastes and loads chunks while the
	# simulation is idle, and starts the next tick when one is due.
	_tick_accum += delta
	if _tick_running and sim.is_tick_done():
		var r: Dictionary = sim.finish_tick()
		last_tick_ms = r["ms_per_tick"]
		active_chunks = int(r["active_chunks_per_tick"])
		scanned_max = maxi(scanned_max, int(r.get("scanned_cells_per_tick", 0)))
		ticks_total += 1
		_tick_running = false
		_collect_dirty()
		if pinned:
			_pin_quiet = _pin_quiet + 1 if active_chunks == 0 and columns_loaded > 0 else 0
	if _tick_running:
		return
	if pinned:
		_pin_age += delta
		if _pin_quiet >= PIN_QUIET_TICKS or _pin_age >= PIN_MAX_S:
			release()
			return
	else:
		_move_window()
	_load_pending()
	if not _push_queue.is_empty() and not hold_pushes:
		var t1 := Time.get_ticks_usec()
		_drain_push(Settings.sim_push_budget_ms * (PIN_PUSH_SHARE if pinned else 1.0))
		push_ms = (Time.get_ticks_usec() - t1) / 1000.0
		_frame_cost()["push_ms"] += push_ms
	var period := 1.0 / Settings.sim_tick_hz
	if _tick_accum >= period:
		_tick_accum = minf(_tick_accum - period, period * 2.0)   # never build up more than two ticks of debt
		var t0 := Time.get_ticks_usec()
		if pieces:
			pieces.tick(period)   # the SimWorld is idle here: pieces crush, snap and settle into its cells
		sim.set_focus(Vector3i(_player.floor()) - origin * Settings.CHUNK)
		_tick_running = sim.start_tick()
		_frame_cost()["tick_ms"] += (Time.get_ticks_usec() - t0) / 1000.0   # main-thread cost of starting it
	if _fire_test > 0:
		_run_fire_test()


## The player's position (world voxels); the window follows it when idle.
func follow(center_world: Vector3) -> void:
	_player = center_world


func _move_window() -> void:
	var off := _player / Settings.CHUNK - Vector3(origin) - Vector3(size_chunks) * CENTER
	if absf(off.x) <= SLACK.x and absf(off.y) <= SLACK.y and absf(off.z) <= SLACK.z:
		return
	var want := _target(_player)
	var d := want - origin
	var t0 := Time.get_ticks_usec()
	var leaving_active := _leaving_active(d)
	_flush_leaving(d)   # chunks sliding out of the window must reach the terrain first
	var t1 := Time.get_ticks_usec()
	sim.shift(d)        # wrapped: an offset moves; the vacated chunks are walls until loaded
	var old := origin
	origin = want
	var full := (1 << size_chunks.y) - 1
	# columns that left the window are forgotten
	for k in _pending.keys():
		if k.x < origin.x or k.y < origin.z or k.x >= origin.x + size_chunks.x or k.y >= origin.z + size_chunks.z:
			_pending.erase(k)
			_retry_at.erase(k)
	if d.y != 0:
		# every column that stays shifts its mask; the rows that entered at the top or bottom wait
		var n := mini(absi(d.y), size_chunks.y)
		var rows := ((1 << n) - 1) << (size_chunks.y - n) if d.y > 0 else (1 << n) - 1
		for z in size_chunks.z:
			for x in size_chunks.x:
				var k := Vector2i(origin.x + x, origin.z + z)
				var m := int(_pending.get(k, 0))
				m = (m >> d.y) & full if d.y > 0 else (m << -d.y) & full
				_pending[k] = m | rows
	# columns that entered the window wait whole
	for z in size_chunks.z:
		var wz := origin.z + z
		var in_z := wz >= old.z and wz < old.z + size_chunks.z
		for x in size_chunks.x:
			var wx := origin.x + x
			if in_z and wx >= old.x and wx < old.x + size_chunks.x:
				continue
			_pending[Vector2i(wx, wz)] = full
	if not leaving_active.is_empty():
		_pin.call_deferred(leaving_active)   # in a frame of its own (a pin sets up a SimWorld and a viewer)
	for p in pins:
		p.wall_off()
	var cost := _frame_cost()
	cost["flush_ms"] += (t1 - t0) / 1000.0
	cost["shift_ms"] += (Time.get_ticks_usec() - t1) / 1000.0
	shifts += 1


## Loads waiting chunks from the terrain, the column nearest the player first,
## one copy per contiguous run of waiting chunks, within
## Settings.sim_load_budget_ms (at least one per frame). Columns whose terrain is
## not streamed in yet wait.
const NEAREST := 8
const RETRY_MS := 250
var _retry_at := {}   # column -> msec before which it is not tried again (its terrain was not in)


func _load_pending() -> void:
	if _pending.is_empty():
		return
	var t0 := Time.get_ticks_usec()
	var now := Time.get_ticks_msec()
	var budget_us := int(Settings.sim_load_budget_ms * 1000.0)
	var pc := Vector2(_player.x, _player.z) / Settings.CHUNK - Vector2(0.5, 0.5)
	# the NEAREST waiting columns, nearest first, from one pass over the queue (cheaper than
	# keeping it sorted; columns whose terrain has not streamed in yet are simply passed over)
	var near: Array[Vector2i] = []
	var near_d := PackedFloat32Array()
	for k in _pending:
		if now < int(_retry_at.get(k, 0)):
			continue   # its terrain was not in a moment ago: let the next ones load meanwhile
		var dd := (Vector2(k) - pc).length_squared()
		if near.size() == NEAREST and dd >= near_d[NEAREST - 1]:
			continue
		var at := near.size()
		while at > 0 and near_d[at - 1] > dd:
			at -= 1
		near.insert(at, k)
		near_d.insert(at, dd)
		if near.size() > NEAREST:
			near.resize(NEAREST)
			near_d.resize(NEAREST)
	var n := 0
	for best in near:
		if n > 0 and Time.get_ticks_usec() - t0 >= budget_us:
			break
		var m := int(_pending[best])
		if m == 0:
			_pending.erase(best)
			continue
		if pinned:
			m &= ~_owner_rows(best)   # the main window holds those: walls here
			if m == 0:
				continue
		# the waiting chunk nearest the player's height whose terrain is in, grown up and down through waiting
		# chunks whose terrain is in too (ADR 0050: a 64 m window reaches past the terrain's data above and below)
		var py := int(floor(_player.y / Settings.CHUNK)) - origin.y
		var lo := -1
		for dk in size_chunks.y * 2:
			var k := py + (dk + 1) / 2 * (1 if dk % 2 == 1 else -1)
			if k >= 0 and k < size_chunks.y and (m >> k) & 1 == 1 and _terrain_in(best, k):
				lo = k
				break
		if lo < 0:
			_retry_at[best] = now + RETRY_MS   # no terrain data there yet (or ever: beyond full detail)
			continue
		var hi := lo + 1
		while hi < size_chunks.y and (m >> hi) & 1 == 1 and _terrain_in(best, hi):
			hi += 1
		while lo > 0 and (m >> (lo - 1)) & 1 == 1 and _terrain_in(best, lo - 1):
			lo -= 1
		var w0 := Vector3i(best.x, origin.y + lo, best.y)
		var bytes: PackedByteArray = world.read_column(w0, hi - lo)
		sim.set_column_bytes(Vector3i(best.x - origin.x, lo, best.y - origin.z), hi - lo, bytes, 2)
		m = int(_pending[best]) & ~(((1 << (hi - lo)) - 1) << lo)
		if m == 0:
			_pending.erase(best)
		else:
			_pending[best] = m
		chunks_loaded += hi - lo
		columns_loaded += 1
		n += 1
	if n > 0:
		var c := _frame_cost()
		c["load_ms"] += (Time.get_ticks_usec() - t0) / 1000.0
		c["loads"] += n


## ADR 0050: the active chunks (world coords) that a move by d takes out of the window.
func _leaving_active(d: Vector3i) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	if pins.size() >= PIN_MAX:
		return out
	# active and changed in the last RECENT_TICKS ticks: a chunk that only loaded is active once, and is no mover
	for wc in _recent.keys():
		if ticks_total - int(_recent[wc]) > RECENT_TICKS:
			_recent.erase(wc)
	if _recent.is_empty():
		return out
	for lc in sim.get_active_chunks():
		var wc: Vector3i = lc + origin
		if _recent.has(wc) and not is_chunk_in_window(wc - origin - d) and not _pinned_at(wc):
			out.append(wc)
	return out


func _pinned_at(wc: Vector3i) -> bool:
	for p in pins:
		if p.is_chunk_in_window(wc - p.origin):
			return true
	return false


## A pin around what still moves (centred on the leaving active chunks).
func _pin(active: Array[Vector3i]) -> void:
	var c := Vector3.ZERO
	for wc in active:
		c += Vector3(wc)
	c = c / active.size() + Vector3(0.5, 0.5, 0.5)
	var t0 := Time.get_ticks_usec()
	var p := SimBubble.new()
	p.name = "SimPin%d" % pins_made
	add_child(p)
	p.setup(world, c * Settings.CHUNK, self)
	p.hold_pushes = false
	pins.append(p)
	pins_made += 1
	pin_ms_max = maxf(pin_ms_max, (Time.get_ticks_usec() - t0) / 1000.0)
	_frame_cost()["edit_ms"] += (Time.get_ticks_usec() - t0) / 1000.0


## A pin: bit k set = local chunk row k of column `col` (world chunk x, z) lies inside the main window.
func _owner_rows(col: Vector2i) -> int:
	var o := owner_bubble
	if col.x < o.origin.x or col.y < o.origin.z or col.x >= o.origin.x + o.size_chunks.x or col.y >= o.origin.z + o.size_chunks.z:
		return 0
	var m := 0
	for k in size_chunks.y:
		var wy := origin.y + k
		if wy >= o.origin.y and wy < o.origin.y + o.size_chunks.y:
			m |= 1 << k
	return m


## A pin, after the main window moved: chunks it now covers are pushed to the terrain and become walls here (they
## wait, and load again once the main window leaves them).
func wall_off() -> void:
	if _tick_running:
		sim.finish_tick()
		_tick_running = false
		_collect_dirty()
	for z in size_chunks.z:
		for x in size_chunks.x:
			var col := Vector2i(origin.x + x, origin.z + z)
			var rows := _owner_rows(col)
			if rows == 0:
				continue
			var m := int(_pending.get(col, 0))
			for k in size_chunks.y:
				if rows & (1 << k) and not m & (1 << k):
					var wc := Vector3i(col.x, origin.y + k, col.y)
					if _push_queue.has(wc):
						_push_one(wc)
						_push_queue.erase(wc)
					sim.set_chunk_loaded(wc - origin, false)
			_pending[col] = m | rows


## A pin lets go: everything reaches the terrain, then it frees itself.
func release() -> void:
	flush_all()
	if owner_bubble and not _worst.is_empty() and float(_worst[0]["total_ms"]) > float(owner_bubble._pin_worst.get("total_ms", 0.0)):
		owner_bubble._pin_worst = _worst[0]
	_ready = false
	if owner_bubble:
		owner_bubble.pins.erase(self)
	queue_free()


func _terrain_in(col: Vector2i, k: int) -> bool:
	return world.is_box_loaded(Vector3i(col.x, origin.y + k, col.y) * Settings.CHUNK, Vector3i.ONE * Settings.CHUNK)


## Records a slow frame with Godot's own monitors (previous frame) next to the
## bubble's costs, so a spike can be attributed without a profiler.
func _note_slow_frame(delta: float) -> void:
	var prev: Dictionary = _cost if _cost.get("frame", -1) == Engine.get_process_frames() - 1 else {}
	var t: Dictionary = world.terrain.get_statistics()
	_slow.append({"frame_ms": snappedf(delta * 1000.0, 0.01),
		"process_ms": snappedf(Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0, 0.01),
		"physics_ms": snappedf(Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS) * 1000.0, 0.01),
		"draw_calls": Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		"vram_mb": snappedf(Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0, 1),
		"bubble_ms": prev.get("total_ms", 0.0), "pushed": prev.get("pushed", 0), "loads": prev.get("loads", 0),
		"terrain_update_ms": t.get("time_update_task", 0), "terrain_mesh_req": t.get("time_mesh_requests", 0)})
	_slow.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a["frame_ms"] > b["frame_ms"])
	if _slow.size() > 8:
		_slow.resize(8)


## Moves the sim's dirty chunks into the push queue (world coords, so they
## survive window moves).
func _collect_dirty() -> void:
	var changed: Array = []
	for lc in sim.get_dirty_chunks():
		var wc: Vector3i = lc + origin
		if not _is_unloaded(wc):   # never paste a chunk before it was loaded
			_push_queue[wc] = true
			changed.append(wc)
			if not pinned:
				_recent[wc] = ticks_total
	if not changed.is_empty():
		chunks_changed.emit(changed)
	sim.clear_dirty()
	push_queue_max = maxi(push_queue_max, _push_queue.size())


## Pastes queued chunks into the terrain, oldest first, until `budget_ms` of
## main-thread time is used (at least one per call). Each paste costs a copy
## plus a remesh; unbudgeted, a big fire turned every tick into a long frame (ADR 0010).
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
		if not _push_one(wc):
			continue   # its terrain block streamed out: keep it until it is back or leaves the window
		_last_push[wc] = now
		done.append(wc)
	for wc in done:
		_push_queue.erase(wc)


## False while the chunk's terrain is not loaded: a paste there is dropped, and an edit to a block
## that unloads before godot_voxel flushes it used to crash the game (bug report 2026-10-24).
func _push_one(wc: Vector3i) -> bool:
	if not is_chunk_in_window(wc - origin):
		return true   # left the window (flushed on the move already)
	if not world.is_area_loaded(wc):
		return false
	world.write_chunk(wc, sim.get_chunk_bytes(wc - origin, 2))
	chunks_pushed += 1
	_frame_cost()["pushed"] += 1
	return true


## Before moving by d: push everything queued that is about to leave the
## window; the rest stays queued and is pushed from its new place.
func _flush_leaving(d: Vector3i) -> void:
	_collect_dirty()
	var leaving: Array[Vector3i] = []
	for wc in _push_queue:
		if not is_chunk_in_window(wc - origin - d):
			leaving.append(wc)
	for wc in leaving:
		_push_one(wc)
		_push_queue.erase(wc)


## Pushes everything now (tests, saving, quitting).
func flush_all() -> void:
	for p in pins:
		p.flush_all()
	if _tick_running:
		sim.finish_tick()
		_tick_running = false
	_collect_dirty()
	_drain_push(1e9)


## Debug (fire_test=N on the command line): once the chunks around the player
## have loaded (6 s after start), ignite every burnable voxel within N voxels.
func _run_fire_test() -> void:
	if _fire_test_at < 0.0:
		_fire_test_at = Time.get_ticks_msec() / 1000.0 + 6.0
		return
	if Time.get_ticks_msec() / 1000.0 < _fire_test_at:
		return
	var c: Vector3i = Vector3i(_player.floor()) - origin * Settings.CHUNK
	var r := Vector3i.ONE * _fire_test
	fire_ignited += sim.ignite_box(c - r, c + r + Vector3i.ONE)
	print("fire_test: ignited %d voxels around %s" % [fire_ignited, Vector3i(_player.floor())])
	_fire_test = 0


## Mass edit (explosion, big dig; ADR 0012 requires these never to choke a frame):
## every voxel whose centre lies within `radius` voxels of `center` (world voxel
## coords) becomes `material`, in one native call. Stations and bedrock stay.
## Only the window is edited; chunks still loading are skipped. The changed
## chunks reach the terrain through the budgeted push queue, and the kernel
## loosens static ground around the hole (ADR 0013), so craters collapse.
## Returns the number of voxels changed. `only` (the world vacuum, ADR 0056): just these materials
## change, everything else stays.
func blast(center: Vector3, radius: float, material := GameMaterials.AIR, only := PackedInt32Array()) -> int:
	if not _ready:
		return 0
	if _keep.is_empty():
		var defs := GameMaterials.defs()
		for i in defs.size():
			var d: Dictionary = defs[i]
			if d.has("interact") or d.has("entity") or float(d.get("hp", 0)) >= 1e6:
				_keep.append(i + 1)
	if _tick_running:
		sim.finish_tick()   # edits wait for the running tick (a few ms at worst)
		_tick_running = false
		_collect_dirty()
	var t0 := Time.get_ticks_usec()
	var keep := _keep
	if not only.is_empty():
		keep = PackedInt32Array()
		for i in range(1, mini(GameMaterials.defs().size() + 1, 256)):
			if not only.has(i) or _keep.has(i):
				keep.append(i)
	var n := sim.fill_sphere(center - Vector3(origin * Settings.CHUNK), radius, material, keep)
	if pieces and material == GameMaterials.AIR:
		pieces.carve(center, radius)   # pieces in flight take the cut too
	var ms := (Time.get_ticks_usec() - t0) / 1000.0
	_frame_cost()["edit_ms"] = _frame_cost().get("edit_ms", 0.0) + ms
	blasts += 1
	blast_voxels += n
	blast_ms_max = maxf(blast_ms_max, ms)
	return n


## Debug mass edit (/fill): sets every voxel of the world box [wmin, wmax) to
## `material`. Fails (returns -1) unless the whole box lies inside the window
## and every chunk it touches is loaded. Returns the box volume.
func fill_box(wmin: Vector3i, wmax: Vector3i, material: int) -> int:
	if not _ready:
		return -1
	var lo := Vector3i((Vector3(wmin) / Settings.CHUNK).floor())
	var hi := Vector3i((Vector3(wmax - Vector3i.ONE) / Settings.CHUNK).floor())
	if not is_chunk_in_window(lo - origin) or not is_chunk_in_window(hi - origin):
		return -1
	for z in range(lo.z, hi.z + 1):
		for y in range(lo.y, hi.y + 1):
			for x in range(lo.x, hi.x + 1):
				if _is_unloaded(Vector3i(x, y, z)):
					return -1
	if _tick_running:
		sim.finish_tick()
		_tick_running = false
		_collect_dirty()
	var t0 := Time.get_ticks_usec()
	var o := origin * Settings.CHUNK
	sim.fill_box(wmin - o, wmax - o, material)
	_frame_cost()["edit_ms"] = _frame_cost().get("edit_ms", 0.0) + (Time.get_ticks_usec() - t0) / 1000.0
	var d := wmax - wmin
	return d.x * d.y * d.z


## Player edits go through the sim when inside the bubble (so the material
## simulation reacts immediately), otherwise straight to the terrain.
func set_voxel(wp: Vector3i, material: int) -> bool:
	var wc := Vector3i((Vector3(wp) / Settings.CHUNK).floor())
	if not _ready or not is_chunk_in_window(wc - origin) or _is_unloaded(wc):
		return false
	sim.set_voxel(wp - origin * Settings.CHUNK, material)
	return true


## The bubble's current material at a world voxel, or -1 when the voxel is
## outside the window or its chunk is still loading. Newer than the terrain:
## edits reach the terrain only on the next push.
func get_voxel(wp: Vector3i) -> int:
	if not _ready:
		return -1
	var wc := Vector3i((Vector3(wp) / Settings.CHUNK).floor())
	if not is_chunk_in_window(wc - origin) or _is_unloaded(wc):
		return -1
	return sim.get_voxel(wp - origin * Settings.CHUNK)


## Random cells around `center_world` for ambient particles (ADR 0026,
## SimWorld.sample_cells): 7 ints per hit (x, y, z in world voxels, material,
## material above, material below, flags). Empty while a tick runs (see can_sample).
func sample_cells(center_world: Vector3i, radius: int, count: int, rng_seed: int, mask: PackedByteArray) -> PackedInt32Array:
	if not _ready:
		return PackedInt32Array()
	var o := origin * Settings.CHUNK
	var r := sim.sample_cells(center_world - o, radius, count, rng_seed, mask)
	for i in range(0, r.size(), 7):
		r[i] += o.x
		r[i + 1] += o.y
		r[i + 2] += o.z
	return r


## True when the local chunk `lc` lies inside the window.
func is_chunk_in_window(lc: Vector3i) -> bool:
	return lc.x >= 0 and lc.y >= 0 and lc.z >= 0 and lc.x < size_chunks.x and lc.y < size_chunks.y and lc.z < size_chunks.z


## False while a simulation tick runs on its thread (cells cannot be read).
func can_sample() -> bool:
	return _ready and sim.is_tick_done()


## Debug: compare loaded chunks near the player with what the generator says
## they should be. In a settled, unedited world they must match; mismatches mean
## chunks were loaded while the terrain was still streaming or the sim moved something.
func verify_against_generator(gen: FineGen, radius_chunks := 3) -> Dictionary:
	if not _ready:
		return {}
	var mismatched := 0
	var air_where_solid := 0
	var solid_where_air := 0
	var checked := 0
	var examples := []
	var pc := Vector3i((_player / Settings.CHUNK).floor())
	for z in range(-radius_chunks, radius_chunks + 1):
		for y in range(-radius_chunks, radius_chunks + 1):
			for x in range(-radius_chunks, radius_chunks + 1):
				var wc := pc + Vector3i(x, y, z)
				if not is_chunk_in_window(wc - origin) or _is_unloaded(wc):
					continue
				var expect: PackedByteArray = gen.generate_block(wc * Settings.CHUNK, Vector3i(16, 16, 16), 0, 2)
				var got: PackedByteArray = sim.get_chunk_bytes(wc - origin, 2)
				checked += 1
				if expect.is_empty():
					expect = PackedByteArray()
					expect.resize(8192)
				if expect == got:
					continue
				mismatched += 1
				var aws := 0
				var swa := 0
				var pairs := {}
				for i in 4096:
					var e := expect[i * 2] | (expect[i * 2 + 1] << 8)
					var g := got[i * 2] | (got[i * 2 + 1] << 8)
					if e != 0 and g == 0:
						aws += 1
					elif e == 0 and g != 0:
						swa += 1
					if e != g and examples.size() < 4:
						var key := "%s->%s" % [GameMaterials.name_of(e), GameMaterials.name_of(g)]
						pairs[key] = pairs.get(key, 0) + 1
				air_where_solid += aws
				solid_where_air += swa
				if examples.size() < 4:
					examples.append({"world_chunk": wc, "air_where_solid": aws, "solid_where_air": swa, "pairs": pairs})
	return {"checked": checked, "mismatched": mismatched, "air_where_solid": air_where_solid, "solid_where_air": solid_where_air, "examples": examples}


## True while no tick runs in the background: the simulation may be read without waiting.
func is_idle() -> bool:
	return _ready and not _tick_running


## True when the local chunk `lc` holds terrain data (not waiting to load).
func is_chunk_loaded(lc: Vector3i) -> bool:
	return _ready and is_chunk_in_window(lc) and not _is_unloaded(lc + origin)


## True once every chunk within the near part of the window holds terrain data
## (the loading screen and DebugCapture wait for it; the far columns keep loading).
## `r` chunks around the player count; a teleport cover passes a smaller one (ADR 0066).
func is_loaded(r := 4) -> bool:
	if not _ready:
		return false
	if _pending.is_empty():
		return true
	var pc := Vector3i((_player / Settings.CHUNK).floor())
	var near := 0   # local y rows within mini(r, 3) chunks of the player
	var ry := mini(r, 3)
	for y in range(pc.y - origin.y - ry, pc.y - origin.y + ry + 1):
		if y >= 0 and y < size_chunks.y:
			near |= 1 << y
	for k in _pending:
		if absi(k.x - pc.x) <= r and absi(k.y - pc.z) <= r and int(_pending[k]) & near != 0:
			return false
	return true


func _count_unloaded() -> int:
	var n := 0
	for k in _pending:
		var m := int(_pending[k])
		while m != 0:
			m &= m - 1
			n += 1
	return n


var _burning := -1
var _burning_at := -1000

## Burning cells in the window for the stats (F3, the autopilot JSON): count_burning is a full pass over
## the window, so it is taken at most every 30 ticks and only when stats are asked for, never in play.
func _burning_count() -> int:
	if _fire_test > 0 or not can_sample():
		return _burning
	if ticks_total - _burning_at >= 30:
		_burning_at = ticks_total
		_burning = sim.count_burning()
	return _burning


func stats() -> Dictionary:
	if not _ready:
		return {}
	for p in pins:
		if not p._worst.is_empty() and float(p._worst[0]["total_ms"]) > float(_pin_worst.get("total_ms", 0.0)):
			_pin_worst = p._worst[0]
	return {
		"origin_chunk": origin, "size_chunks": size_chunks, "unloaded_chunks": _count_unloaded(), "pending_columns": _pending.size(),
		"tick_ms": snappedf(last_tick_ms, 0.01), "ticks": ticks_total, "active_chunks": active_chunks, "scanned_max": scanned_max,
		"chunks_pushed": chunks_pushed, "chunks_loaded": chunks_loaded, "columns_loaded": columns_loaded, "push_ms": snappedf(push_ms, 0.01),
		"shifts": shifts, "worst_frames": _worst, "slow_frames": _slow, "push_queue": _push_queue.size(), "push_queue_max": push_queue_max,
		"burning": _burning_count(), "fire_ignited": fire_ignited,
		"blasts": blasts, "blast_voxels": blast_voxels, "blast_ms_max": snappedf(blast_ms_max, 0.01),
		"pins": pins.size(), "pins_made": pins_made, "pin_ms_max": snappedf(pin_ms_max, 0.01), "pin_worst": _pin_worst, "still_woken": sim.get_stats().get("still_woken", 0),
	}
