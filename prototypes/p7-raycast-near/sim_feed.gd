class_name SimFeed
extends RefCounted

## P8b: the ray-cast volume fed by the native material simulation (SimWorld
## from the technosim extension) instead of the compute fill. Each tick the
## dirty 16^3 chunks are read as raw bytes, concatenated into a storage buffer
## and scattered into the volume texture by scatter.glsl (one workgroup per
## chunk). Also fires a "1000 TNT" event (a box carved to air so the powder
## rim collapses) and a steady spring so there is always something moving.

const CS := 16

var n := 256
var hz := 30.0
var threads := 12
var tnt_at := 4.0
var tnt_m := 8.0            # side of the box carved by the TNT event, in metres
var cell_m := 0.0625

var sim: SimWorld
var _rd: RenderingDevice
var _tex_rid: RID
var _occ_cb: Callable
var _shader: RID
var _pipeline: RID
var _bytes_buf: RID
var _chunks_buf: RID
var _uset: RID
var _capacity := 0
var _accum := 0.0
var _tnt_done := false
var _ticks_after_tnt := 0
var _tick_ms: PackedFloat32Array = []
var _tick_ms_after_tnt: PackedFloat32Array = []
var _dirty_counts: PackedInt32Array = []
var _upload_ms: PackedFloat32Array = []
var _tnt_fill_ms := 0.0
var _tnt_dirty_max := 0
var _active_max := 0
var _spring := Vector3i.ZERO


func setup(rd: RenderingDevice, tex_rid: RID, occ_cb: Callable, p_n: int, p_cell_m: float) -> void:
	_rd = rd
	_tex_rid = tex_rid
	_occ_cb = occ_cb
	n = p_n
	cell_m = p_cell_m
	var chunks := n / CS
	sim = SimWorld.new()
	sim.configure(Vector3i(chunks, chunks, chunks), threads, 7)
	sim.set_materials([
		{"name": "stone", "cell_type": "solid", "density": 10},
		{"name": "dirt", "cell_type": "liquid", "liquid_sand": true, "liquid_static": true, "density": 3},
		{"name": "grass", "cell_type": "liquid", "liquid_sand": true, "liquid_static": true, "density": 3},
		{"name": "sand", "cell_type": "liquid", "liquid_sand": true, "density": 3},
		{"name": "water", "cell_type": "liquid", "density": 1, "liquid_flow_speed": 1.0},
		{"name": "glow", "cell_type": "solid", "density": 10},
	])
	sim.set_reactions([])
	sim.mark_all_loaded(true)
	_fill()
	# scatter pipeline; buffers sized for every chunk (the first upload is the whole volume)
	_capacity = chunks * chunks * chunks
	var sf: RDShaderFile = load("res://scatter.glsl")
	_shader = _rd.shader_create_from_spirv(sf.get_spirv())
	_pipeline = _rd.compute_pipeline_create(_shader)
	_bytes_buf = _rd.storage_buffer_create(_capacity * CS * CS * CS)
	_chunks_buf = _rd.storage_buffer_create(_capacity * 16)
	var u0 := RDUniform.new()
	u0.uniform_type = RenderingDevice.UNIFORM_TYPE_IMAGE
	u0.binding = 0
	u0.add_id(_tex_rid)
	var u1 := RDUniform.new()
	u1.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
	u1.binding = 1
	u1.add_id(_bytes_buf)
	var u2 := RDUniform.new()
	u2.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
	u2.binding = 2
	u2.add_id(_chunks_buf)
	_uset = _rd.uniform_set_create([u0, u1, u2], _shader, 0)
	# first upload: everything
	var all: Array = []
	for z in chunks:
		for y in chunks:
			for x in chunks:
				all.append(Vector3i(x, y, z))
	_upload(all)
	sim.clear_dirty()


## Ground with a hill, a basin with water, a sand beach, glowing ore in the rock.
func _fill() -> void:
	var g := int(n * 0.40)
	sim.fill_box(Vector3i(0, 0, 0), Vector3i(n, g - 8, n), 1)                       # stone
	sim.fill_box(Vector3i(0, g - 8, 0), Vector3i(n, g, n), 2)                       # dirt
	sim.fill_box(Vector3i(0, g, 0), Vector3i(n, g + 2, n), 3)                       # grass
	var hx := int(n * 0.6)
	var hz2 := int(n * 0.55)
	var hr := int(n * 0.18)
	for i in 6:                                                                     # a stepped hill
		var r := hr - i * hr / 6
		var y0 := g + 2 + i * 3
		sim.fill_box(Vector3i(hx - r, y0, hz2 - r), Vector3i(hx + r, y0 + 3, hz2 + r), 2)
		sim.fill_box(Vector3i(hx - r, y0 + 3, hz2 - r), Vector3i(hx + r, y0 + 4, hz2 + r), 3)
	var bx := int(n * 0.22)
	var bz := int(n * 0.3)
	var br := int(n * 0.14)
	var depth := int(n * 0.06)
	sim.fill_box(Vector3i(bx - br, g - depth, bz - br), Vector3i(bx + br, g + 2, bz + br), 0)      # basin
	sim.fill_box(Vector3i(bx - br, g - depth, bz - br), Vector3i(bx + br, g - 2, bz + br), 5)      # water
	sim.fill_box(Vector3i(bx - br - 6, g, bz - br - 6), Vector3i(bx + br + 6, g + 2, bz + br + 6), 4) # sand rim
	sim.fill_box(Vector3i(bx - br, g - depth, bz - br), Vector3i(bx + br, g - 2, bz + br), 5)      # water again over the sand fill
	sim.scatter(Vector3i(0, 0, 0), Vector3i(n, g - 10, n), 6, 0.004, 3)                              # glow ore
	_spring = Vector3i(hx, g + 2 + 6 * 3 + 2, hz2)


func _upload(dirty: Array) -> int:
	var count := mini(dirty.size(), _capacity)
	if count == 0:
		return 0
	var bytes := PackedByteArray()
	var coords := PackedInt32Array()
	coords.resize(count * 4)
	for i in count:
		var c: Vector3i = dirty[i]
		bytes.append_array(sim.get_chunk_bytes(c, 1))  # native copy, no per-byte GDScript loop
		coords[i * 4] = c.x
		coords[i * 4 + 1] = c.y
		coords[i * 4 + 2] = c.z
	_rd.buffer_update(_bytes_buf, 0, bytes.size(), bytes)
	var cbytes := coords.to_byte_array()
	_rd.buffer_update(_chunks_buf, 0, cbytes.size(), cbytes)
	var pc := PackedByteArray()
	pc.resize(16)
	pc.encode_s32(0, n)
	pc.encode_s32(4, count)
	var cl := _rd.compute_list_begin()
	_rd.compute_list_bind_compute_pipeline(cl, _pipeline)
	_rd.compute_list_bind_uniform_set(cl, _uset, 0)
	_rd.compute_list_set_push_constant(cl, pc, pc.size())
	_rd.compute_list_dispatch(cl, count, 1, 1)
	_rd.compute_list_end()
	_occ_cb.call(0, n / 8)
	return count


## Called every frame; ticks the simulation at `hz` and pushes its dirty chunks.
func step(delta: float, elapsed: float) -> void:
	_accum += delta
	if _accum < 1.0 / hz:
		return
	_accum = fmod(_accum, 1.0 / hz)
	# a spring on the hill keeps water flowing
	for i in 6:
		sim.set_voxel(_spring + Vector3i(i % 3 - 1, 0, i / 3 - 1), 5)
	if not _tnt_done and elapsed >= tnt_at:
		_tnt_done = true
		var r := int(tnt_m / cell_m / 2.0)
		var c := Vector3i(int(n * 0.6), int(n * 0.40) + 2, int(n * 0.55))
		var t0 := Time.get_ticks_usec()
		sim.fill_box(c - Vector3i(r, r, r), c + Vector3i(r, r, r), 0)
		_tnt_fill_ms = (Time.get_ticks_usec() - t0) / 1000.0
		_ticks_after_tnt = 60
	var t1 := Time.get_ticks_usec()
	sim.tick(1)
	var tick_ms := (Time.get_ticks_usec() - t1) / 1000.0
	_tick_ms.append(tick_ms)
	if _ticks_after_tnt > 0:
		_tick_ms_after_tnt.append(tick_ms)
		_ticks_after_tnt -= 1
	_active_max = maxi(_active_max, sim.get_active_chunk_count())
	var t2 := Time.get_ticks_usec()
	var dirty: Array = sim.get_dirty_chunks()
	var count := _upload(dirty)
	sim.clear_dirty()
	_upload_ms.append((Time.get_ticks_usec() - t2) / 1000.0)
	_dirty_counts.append(count)
	if _tnt_done and _ticks_after_tnt > 0:
		_tnt_dirty_max = maxi(_tnt_dirty_max, count)


func _pct(arr: PackedFloat32Array, q: float) -> float:
	var a := arr.duplicate()
	a.sort()
	return a[clampi(int(q * a.size()), 0, a.size() - 1)] if a.size() > 0 else 0.0


func stats() -> Dictionary:
	var dc := PackedFloat32Array()
	for v in _dirty_counts:
		dc.append(float(v))
	return {
		"sim_hz": hz, "sim_threads": threads, "sim_chunks": (n / CS) * (n / CS) * (n / CS), "ticks": _tick_ms.size(),
		"tick_ms_p50": snappedf(_pct(_tick_ms, 0.5), 0.01), "tick_ms_p99": snappedf(_pct(_tick_ms, 0.99), 0.01), "tick_ms_max": snappedf(_pct(_tick_ms, 1.0), 0.01),
		"active_chunks_max": _active_max,
		"dirty_chunks_p50": int(_pct(dc, 0.5)), "dirty_chunks_max": int(_pct(dc, 1.0)),
		"upload_ms_p50": snappedf(_pct(_upload_ms, 0.5), 0.01), "upload_ms_p99": snappedf(_pct(_upload_ms, 0.99), 0.01), "upload_ms_max": snappedf(_pct(_upload_ms, 1.0), 0.01),
		"tnt_box_m": tnt_m, "tnt_fill_ms": snappedf(_tnt_fill_ms, 0.01),
		"tick_ms_after_tnt_p50": snappedf(_pct(_tick_ms_after_tnt, 0.5), 0.01), "tick_ms_after_tnt_max": snappedf(_pct(_tick_ms_after_tnt, 1.0), 0.01),
		"tnt_dirty_chunks_max": _tnt_dirty_max,
		"sim_stats": sim.get_stats(),
	}


func free_rids() -> void:
	for rid in [_uset, _pipeline, _shader, _bytes_buf, _chunks_buf]:
		if rid.is_valid():
			_rd.free_rid(rid)
