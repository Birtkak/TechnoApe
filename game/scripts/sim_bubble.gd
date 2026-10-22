class_name SimBubble
extends Node

## The Noita-style simulation bubble: a sliding window of chunks around the
## player, owned by the native SimWorld. Chunks entering the window are copied
## from the terrain; the window ticks at Settings.sim_tick_hz; chunks the sim
## changed are pasted back into the terrain. Everything outside is frozen.
##
## Local chunk (0..N) <-> world chunk = local + origin.

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


func setup(p_world: World, center_world: Vector3) -> void:
	world = p_world
	var r := Settings.sim_distance_chunks()
	size_chunks = Vector3i(2 * r + 1, 2 * r + 1, 2 * r + 1)
	sim = SimWorld.new()
	sim.configure(size_chunks, Settings.sim_threads, Settings.world_seed)
	sim.set_materials(GameMaterials.defs())
	sim.set_reactions(GameMaterials.reactions())
	# Chunks stream in over several frames; until loaded they must act as walls,
	# not air, or water next to them flows into nothing (holes in lakes).
	sim.mark_all_loaded(false)
	origin = _center_chunk(center_world) - Vector3i(r, r, r)
	for z in size_chunks.z:
		for y in size_chunks.y:
			for x in size_chunks.x:
				_pending_loads.append(Vector3i(x, y, z))
	_ready = true


func _center_chunk(p: Vector3) -> Vector3i:
	return Vector3i((p / Settings.CHUNK).floor())


func _process(delta: float) -> void:
	if not _ready:
		return
	_load_pending(64)
	_tick_accum += delta
	var period := 1.0 / Settings.sim_tick_hz
	var ticks := 0
	while _tick_accum >= period and ticks < 3:   # catch up at most 3 ticks per frame
		_tick_accum -= period
		var t0 := Time.get_ticks_usec()
		var r: Dictionary = sim.tick(1)
		last_tick_ms = r["ms_per_tick"]
		active_chunks = sim.get_active_chunk_count()
		ticks_total += 1
		ticks += 1
	if ticks > 0:
		var t1 := Time.get_ticks_usec()
		_push_dirty()
		push_ms = (Time.get_ticks_usec() - t1) / 1000.0


## Keep the window centred on the player; shift by whole chunks.
func follow(center_world: Vector3) -> void:
	if not _ready:
		return
	var r := (size_chunks.x - 1) / 2
	var want := _center_chunk(center_world) - Vector3i(r, r, r)
	var d := want - origin
	if d == Vector3i.ZERO:
		return
	# Shift at most one chunk per axis per call to keep spikes small.
	d = d.clamp(Vector3i(-1, -1, -1), Vector3i(1, 1, 1))
	_push_dirty()  # flush before sliding so nothing is lost
	var vacated: Array = sim.shift(d)
	origin += d
	for c in vacated:
		_pending_loads.append(c)


func _load_pending(budget: int) -> void:
	var still: Array[Vector3i] = []
	var n := 0
	for lc in _pending_loads:
		var wc: Vector3i = lc + origin
		if n >= budget or not world.is_area_loaded(wc):
			still.append(lc)
			continue
		var bytes: PackedByteArray = world.read_chunk(wc)
		if bytes.size() != 8192:
			bad_loads += 1
			if bad_loads <= 3:
				push_warning("SimBubble: chunk %s read %d bytes (expected 8192)" % [wc, bytes.size()])
		sim.set_chunk_bytes(lc, bytes, 2)
		chunks_loaded += 1
		n += 1
	_pending_loads = still


func _push_dirty() -> void:
	var dirty: Array = sim.get_dirty_chunks()
	for lc in dirty:
		var wc: Vector3i = lc + origin
		if _pending_loads.has(lc):
			continue  # never pasted before it was loaded
		world.write_chunk(wc, sim.get_chunk_bytes(lc, 2))
		chunks_pushed += 1
	sim.clear_dirty()


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


func stats() -> Dictionary:
	if not _ready:
		return {}
	return {
		"origin_chunk": origin, "size_chunks": size_chunks, "pending_loads": _pending_loads.size(),
		"tick_ms": snappedf(last_tick_ms, 0.01), "ticks": ticks_total, "active_chunks": active_chunks,
		"chunks_pushed": chunks_pushed, "chunks_loaded": chunks_loaded, "bad_loads": bad_loads, "push_ms": snappedf(push_ms, 0.01),
	}
