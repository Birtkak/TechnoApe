class_name MicroBubble
extends RefCounted

## The micro simulation (ADR 0029): a window of 4 x 2 x 4 of the bubble's chunks
## (32 x 16 x 32 m) around the player, simulated at 6.25 cm cells (8 per voxel)
## by a second, toroidal SimWorld. The bubble's chunks inside the window are held:
## they do not tick, and after every micro tick the changed bricks are published,
## as cells into the near field's BrickStore and as their majority into the
## bubble's voxels, which reach the terrain (far field, collision, stains,
## particles) through the bubble's push queue as before. The window moves in whole
## bubble chunks; entering chunks are expanded from the bubble's cells, leaving
## ones are released to it. The window's faces are walls both ways for now.
##
## Everything that touches cells runs while both simulations are idle: SimBubble
## calls update() from its idle section, edits are queued until then.

const SCALE := 8                         # cells per voxel edge (BrickStore.B)
const WINDOW := Vector3i(4, 2, 4)        # bubble chunks
const SIM_CHUNK := 16                    # cells per SimWorld chunk edge
const SLAB := 16                         # cells: big box edits are applied 1 m along z at a time
# Where the player sits in the window, in bubble chunks from its min corner: centred in x/z, low in y
# (things fall: 6-14 m of the window below the feet, 2-10 m above the head when it was placed).
const CENTER := Vector3(2.0, 1.25, 2.0)
const SLACK := Vector3(0.75, 0.65, 0.75)  # the window moves once the player is further than this from CENTER

var bubble: SimBubble
var sim: SimWorld
var origin := Vector3i.ZERO              # world chunk of the window's min corner
var running := false                     # a micro tick runs on its thread
var player := Vector3.ZERO               # world voxels, from SimBubble.follow
var _held := {}                          # world chunk -> true: expanded into the window, held in the bubble
var _pending: Array[Vector3i] = []       # world chunks of the window still to expand
var _edits: Array = []                   # queued until both simulations are idle: [kind, args...]

# stats for the JSON
var ticks := 0
var last_tick_ms := 0.0
var _tick_ms := PackedFloat32Array()
var active_chunks := 0
var scanned_max := 0
var deferred_max := 0
var loads := 0
var load_ms_max := 0.0
var published := 0
var publish_ms_max := 0.0
var publish_pending_max := 0
var shifts := 0
var edit_cells := 0


func setup(p_bubble: SimBubble, center_world: Vector3) -> void:
	bubble = p_bubble
	player = center_world
	sim = SimWorld.new()
	sim.configure(WINDOW * (Settings.CHUNK * SCALE / SIM_CHUNK), Settings.sim_threads, Settings.world_seed)
	if not sim.set_wrapped(true):
		push_error("MicroBubble: window chunk counts must be powers of two")
	sim.set_materials(GameMaterials.defs())
	sim.set_reactions(GameMaterials.reactions())
	sim.set_liquid_fullness(true)
	sim.set_cascade(true)   # a column falls together instead of one 6.25 cm layer per tick
	sim.set_micro_masks(NearField.micro_patterns())   # a tuft of grass is its cells, not a solid 0.5 m box
	var cell_m := Settings.VOXEL_M / SCALE
	sim.set_gravity(9.8 / cell_m / (Settings.sim_tick_hz * Settings.sim_tick_hz))   # 0.174 cells/tick^2 at 30 Hz
	sim.set_lod(int(Settings.micro_lod_m / (cell_m * SIM_CHUNK)), 2)
	sim.set_cell_budget(Settings.micro_cell_budget)
	sim.mark_all_loaded(false)
	origin = _target(center_world)
	_queue_window()


## World cell of the window's local cell (0,0,0).
func world_cell_origin() -> Vector3i:
	return origin * Settings.CHUNK * SCALE


func _target(p: Vector3) -> Vector3i:
	var c := p / Settings.CHUNK - CENTER
	return Vector3i(roundi(c.x), roundi(c.y), roundi(c.z))


## The window stays until the player strays SLACK from CENTER (more than half a
## chunk, so a player on a chunk border does not make it flap).
func _wanted_origin() -> Vector3i:
	var off := player / Settings.CHUNK - Vector3(origin) - CENTER
	if absf(off.x) <= SLACK.x and absf(off.y) <= SLACK.y and absf(off.z) <= SLACK.z:
		return origin
	return _target(player)


func _in_window(wc: Vector3i, o: Vector3i) -> bool:
	var l := wc - o
	return l.x >= 0 and l.y >= 0 and l.z >= 0 and l.x < WINDOW.x and l.y < WINDOW.y and l.z < WINDOW.z


func _queue_window() -> void:
	for z in WINDOW.z:
		for y in WINDOW.y:
			for x in WINDOW.x:
				var wc := origin + Vector3i(x, y, z)
				if not _held.has(wc) and not _pending.has(wc):
					_pending.append(wc)


## Starts a tick on the simulation's thread (the bubble's tick runs beside it).
func start_tick() -> void:
	sim.set_focus(Vector3i(((player - Vector3(origin * Settings.CHUNK)) * SCALE).floor()))
	running = sim.start_tick()


## Collects a finished tick; false while it still runs.
func poll() -> bool:
	if not running:
		return true
	if not sim.is_tick_done():
		return false
	var r: Dictionary = sim.finish_tick()
	running = false
	ticks += 1
	last_tick_ms = r["ms_per_tick"]
	_tick_ms.append(last_tick_ms)
	active_chunks = int(r["active_chunks_per_tick"])
	scanned_max = maxi(scanned_max, int(r["scanned_cells_per_tick"]))
	deferred_max = maxi(deferred_max, int(r["deferred_chunks_per_tick"]))
	return true


## Both simulations idle: apply queued edits, move the window, expand entering
## chunks and publish changed bricks, each under its main-thread budget.
func update() -> Dictionary:
	var cost := {"load_ms": 0.0, "edit_ms": 0.0, "publish_ms": 0.0}
	var t0 := Time.get_ticks_usec()
	if not _edits.is_empty():
		_apply_edits(Settings.micro_load_budget_ms)
		cost["edit_ms"] = (Time.get_ticks_usec() - t0) / 1000.0
	var want := _wanted_origin()
	if want != origin:
		_move(want)
	var t1 := Time.get_ticks_usec()
	_load_pending()
	var t2 := Time.get_ticks_usec()
	cost["load_ms"] = (t2 - t1) / 1000.0
	published += sim.publish(bubble.store, bubble.store_origin_voxel, bubble.sim, world_cell_origin(),
		bubble.origin * Settings.CHUNK, int(Settings.micro_publish_budget_ms * 1000.0))
	var pub := (Time.get_ticks_usec() - t2) / 1000.0
	cost["publish_ms"] = pub
	publish_ms_max = maxf(publish_ms_max, pub)
	publish_pending_max = maxi(publish_pending_max, sim.get_publish_pending())
	return cost


func _move(want: Vector3i) -> void:
	# everything the micro simulation changed must be in the bubble before its chunks are released
	published += sim.publish(bubble.store, bubble.store_origin_voxel, bubble.sim, world_cell_origin(), bubble.origin * Settings.CHUNK, 0)
	for wc in _held.keys():
		if not _in_window(wc, want):
			var lc: Vector3i = wc - bubble.origin
			bubble.sim.set_chunk_held(lc, false)   # out-of-range chunks left the bubble already (flushed there)
			_held.erase(wc)
	var keep: Array[Vector3i] = []
	for wc in _pending:
		if _in_window(wc, want):
			keep.append(wc)
	_pending = keep
	sim.shift((want - origin) * (Settings.CHUNK * SCALE / SIM_CHUNK))
	origin = want
	_queue_window()
	shifts += 1


func _load_pending() -> void:
	if _pending.is_empty():
		return
	var t0 := Time.get_ticks_usec()
	var budget_us := int(Settings.micro_load_budget_ms * 1000.0)
	var still: Array[Vector3i] = []
	var n := 0
	for wc in _pending:
		var lc: Vector3i = wc - bubble.origin
		if (n > 0 and Time.get_ticks_usec() - t0 >= budget_us) or not bubble.is_chunk_in_window(lc) or not bubble.is_chunk_loaded(lc):
			still.append(wc)
			continue
		var t := Time.get_ticks_usec()
		bubble.sim.set_chunk_held(lc, true)
		sim.load_from_coarse(bubble.sim, lc, (wc - origin) * Settings.CHUNK * SCALE)
		load_ms_max = maxf(load_ms_max, (Time.get_ticks_usec() - t) / 1000.0)
		_held[wc] = true
		loads += 1
		n += 1
	_pending = still


## True when the world voxel lies in a chunk the micro simulation owns.
func covers(wp: Vector3i) -> bool:
	return _held.has(Vector3i((Vector3(wp) / Settings.CHUNK).floor()))


func is_loaded() -> bool:
	return _pending.is_empty()


func set_voxel(wp: Vector3i, material: int) -> void:
	_edits.append(["box", wp, wp + Vector3i.ONE, material])


func fill_box(wmin: Vector3i, wmax: Vector3i, material: int) -> void:
	_edits.append(["box", wmin, wmax, material])


func blast(center: Vector3, radius: float, material: int, keep: PackedInt32Array) -> void:
	_edits.append(["sphere", center, radius, material, keep])


func ignite_box(wmin: Vector3i, wmax: Vector3i) -> void:
	_edits.append(["ignite", wmin, wmax])


## Applies queued edits within Settings.micro_load_budget_ms (at least one piece). Boxes (fills, the
## fire test) go in 1 m slabs along z, so a 15 m ignite (12 M cells) spreads over frames.
func _apply_edits(budget_ms := -1.0) -> void:
	var o := origin * Settings.CHUNK
	var t0 := Time.get_ticks_usec()
	var first := true
	while not _edits.is_empty():
		if not first and budget_ms >= 0.0 and Time.get_ticks_usec() - t0 >= budget_ms * 1000.0:
			break
		first = false
		var e: Array = _edits.pop_front()
		match e[0]:
			"box", "ignite":
				var lo: Vector3i = (e[1] - o) * SCALE
				var hi: Vector3i = (e[2] - o) * SCALE
				if hi.z - lo.z > SLAB:
					var rest := e.duplicate()
					rest[1] = Vector3i(e[1].x, e[1].y, e[1].z + SLAB / SCALE)
					_edits.push_front(rest)
					hi.z = lo.z + SLAB
				if e[0] == "box":
					sim.fill_box(lo, hi, e[3])
					var d := hi - lo
					edit_cells += d.x * d.y * d.z
				else:
					edit_cells += sim.ignite_box(lo, hi)
			"sphere":
				var c: Vector3 = (e[1] - Vector3(o)) * SCALE
				edit_cells += sim.fill_sphere(c, e[2] * SCALE, e[3], e[4])


## Writes every changed brick out now (tests, saving, quitting); both simulations must be idle.
func flush() -> void:
	if not _edits.is_empty():
		_apply_edits()
	published += sim.publish(bubble.store, bubble.store_origin_voxel, bubble.sim, world_cell_origin(), bubble.origin * Settings.CHUNK, 0)


func stats() -> Dictionary:
	var s := _tick_ms.duplicate()
	s.sort()
	var p := func(q: float) -> float: return snappedf(s[mini(s.size() - 1, int(s.size() * q))], 0.01) if s.size() > 0 else 0.0
	return {"origin_chunk": origin, "held": _held.size(), "pending": _pending.size(), "ticks": ticks,
		"tick_ms": snappedf(last_tick_ms, 0.01), "tick_ms_p50": p.call(0.5), "tick_ms_p99": p.call(0.99),
		"tick_ms_max": snappedf(s[s.size() - 1], 0.01) if s.size() > 0 else 0.0,
		"active_chunks": active_chunks, "scanned_max": scanned_max, "deferred_max": deferred_max,
		"loads": loads, "load_ms_max": snappedf(load_ms_max, 0.01), "published": published,
		"publish_ms_max": snappedf(publish_ms_max, 0.01), "publish_pending": sim.get_publish_pending(),
		"publish_pending_max": publish_pending_max, "shifts": shifts, "edit_cells": edit_cells,
		"memory_mb": snappedf(float(sim.get_stats()["memory_mb"]), 1)}
