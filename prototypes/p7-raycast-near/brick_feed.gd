class_name BrickFeed
extends RefCounted

## P9 step: the ray-cast volume fed from a BrickStore filled by the real
## generator (uniform bricks) with every surface brick expanded, instead of a
## SimWorld. Same scatter path as SimFeed (16^3 chunk bytes -> compute
## scatter). Each tick a few cells change (a "chisel" walking along the
## surface), at `tnt_at` a 10 m sphere is carved, and the dirty bricks map to
## dirty chunks for the upload.

const CS := 16

var n := 256
var hz := 30.0
var tnt_at := 4.0
var cell_m := 0.0625

var store: BrickStore
var store_bricks := 128          # the store is a 64 m cube; the window (n cells) slides inside it
var win := Vector3i.ZERO         # window origin in world cells (multiple of 16)
var win0 := -1                    # debug: forced initial window origin (cells)
var _slides := 0
var _slide_chunks: PackedInt32Array = []
var _slide_ms: PackedFloat32Array = []
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
var _tick := 0
var _tnt_done := false
var _upload_ms: PackedFloat32Array = []
var _dirty_counts: PackedInt32Array = []
var _tnt_ms := 0.0
var _tnt_chunks := 0
var _fill_ms := 0.0
var _stats_fill := {}


func setup(rd: RenderingDevice, tex_rid: RID, occ_cb: Callable, p_n: int, p_cell_m: float) -> void:
	_rd = rd
	_tex_rid = tex_rid
	_occ_cb = occ_cb
	n = p_n
	cell_m = p_cell_m
	var bricks := store_bricks
	store = BrickStore.new()
	store.configure(Vector3i(bricks, bricks, bricks))
	win = Vector3i.ONE * ((bricks * 8 - n) / 2 / 16 * 16) if win0 < 0 else Vector3i.ONE * win0
	var gen := WorldGen.new()
	gen.configure(1337, 256, 768)
	gen.column_info(0, 0)
	var t0 := Time.get_ticks_usec()
	var y0 := 256 + 40 - bricks / 2
	for bz in range(0, bricks, 16):
		for by in range(0, bricks, 16):
			for bx in range(0, bricks, 16):
				var bytes: PackedByteArray = gen.generate_block(Vector3i(bx - bricks / 2, y0 + by, bz - bricks / 2), Vector3i(16, 16, 16), 0, 2)
				if not bytes.is_empty():
					store.load_voxels(Vector3i(bx, by, bz), Vector3i(16, 16, 16), bytes)
	store.expand_surfaces()
	_fill_ms = (Time.get_ticks_usec() - t0) / 1000.0
	_stats_fill = store.stats()
	store.clear_dirty()
	_capacity = (n / CS) * (n / CS) * (n / CS)
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
	var all: Array = []
	var cpe := n / CS
	var c0 := win / CS
	for z in cpe:
		for y in cpe:
			for x in cpe:
				all.append(c0 + Vector3i(x, y, z))
	_upload(all)


## Window origin in metres (for the volume mesh and the shader).
func origin_m() -> Vector3:
	return Vector3(win) * cell_m


## Slides the window by whole chunks; only the chunks that entered the window are uploaded.
func slide(delta_chunks: Vector3i) -> void:
	if delta_chunks == Vector3i.ZERO:
		return
	var old_c0 := win / CS
	win += delta_chunks * CS
	var c0 := win / CS
	var cpe := n / CS
	var entered: Array = []
	for z in cpe:
		for y in cpe:
			for x in cpe:
				var c := c0 + Vector3i(x, y, z)
				var inside_old := c.x >= old_c0.x and c.x < old_c0.x + cpe and c.y >= old_c0.y and c.y < old_c0.y + cpe and c.z >= old_c0.z and c.z < old_c0.z + cpe
				if not inside_old:
					entered.append(c)
	var t0 := Time.get_ticks_usec()
	var count := _upload(entered)
	_slide_ms.append((Time.get_ticks_usec() - t0) / 1000.0)
	_slide_chunks.append(count)
	_slides += 1


func _upload(chunks: Array) -> int:
	var count := mini(chunks.size(), _capacity)
	if count == 0:
		return 0
	var bytes := PackedByteArray()
	var coords := PackedInt32Array()
	coords.resize(count * 4)
	for i in count:
		var c: Vector3i = chunks[i]
		bytes.append_array(store.get_chunk_bytes(c))
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


## Dirty bricks -> the set of 16^3 chunks (2x2x2 bricks each) to re-upload.
func _dirty_chunks() -> Array:
	var seen := {}
	var c0 := win / CS
	var cpe := n / CS
	for b in store.get_dirty_bricks():
		var c := Vector3i(b.x / 2, b.y / 2, b.z / 2)
		if c.x >= c0.x and c.x < c0.x + cpe and c.y >= c0.y and c.y < c0.y + cpe and c.z >= c0.z and c.z < c0.z + cpe:
			seen[c] = true
	store.clear_dirty()
	return seen.keys()


func step(delta: float, elapsed: float) -> void:
	_accum += delta
	if _accum < 1.0 / hz:
		return
	_accum = fmod(_accum, 1.0 / hz)
	_tick += 1
	# a chisel walking the surface: 64 cells per tick change along a line
	var x := win.x + (n / 2 + _tick) % n
	for k in 64:
		var z := win.z + (n / 2 + k) % n
		var y := win.y + n * 0.5 + 3 * sin(k * 0.3)
		store.set_cell(Vector3i(x, int(y), z), 1 + (k % 2))
	if not _tnt_done and elapsed >= tnt_at:
		_tnt_done = true
		var t0 := Time.get_ticks_usec()
		_tnt_chunks = 0
		store.carve_sphere(win + Vector3i(n / 2, n / 2, n / 2), 80, 0)
		_tnt_ms = (Time.get_ticks_usec() - t0) / 1000.0
	var t1 := Time.get_ticks_usec()
	var dirty := _dirty_chunks()
	var count := _upload(dirty)
	_upload_ms.append((Time.get_ticks_usec() - t1) / 1000.0)
	_dirty_counts.append(count)
	if _tnt_done and _tnt_chunks == 0:
		_tnt_chunks = count


func _f(a: PackedInt32Array) -> PackedFloat32Array:
	var o := PackedFloat32Array()
	for v in a:
		o.append(float(v))
	return o


func _pct(arr: PackedFloat32Array, q: float) -> float:
	var a := arr.duplicate()
	a.sort()
	return a[clampi(int(q * a.size()), 0, a.size() - 1)] if a.size() > 0 else 0.0


func stats() -> Dictionary:
	var dc := PackedFloat32Array()
	for v in _dirty_counts:
		dc.append(float(v))
	return {
		"brick_fill_ms": snappedf(_fill_ms, 0.1), "brick_stats_after_fill": _stats_fill, "brick_stats_end": store.stats(),
		"brick_hz": hz, "brick_ticks": _tick,
		"brick_dirty_chunks_p50": int(_pct(dc, 0.5)), "brick_dirty_chunks_max": int(_pct(dc, 1.0)),
		"brick_upload_ms_p50": snappedf(_pct(_upload_ms, 0.5), 0.01), "brick_upload_ms_p99": snappedf(_pct(_upload_ms, 0.99), 0.01), "brick_upload_ms_max": snappedf(_pct(_upload_ms, 1.0), 0.01),
		"brick_tnt_carve_ms": snappedf(_tnt_ms, 0.01), "brick_tnt_chunks_uploaded": _tnt_chunks,
		"window_slides": _slides, "slide_chunks_max": int(_pct(_f(_slide_chunks), 1.0)), "slide_ms_p50": snappedf(_pct(_slide_ms, 0.5), 0.01), "slide_ms_max": snappedf(_pct(_slide_ms, 1.0), 0.01),
	}


func free_rids() -> void:
	for rid in [_uset, _pipeline, _shader, _bytes_buf, _chunks_buf]:
		if rid.is_valid():
			_rd.free_rid(rid)


## Debug: the first non-air cell below a world cell, walking down.
func scan_surface(wc: Vector3i) -> Array:
	for dy in range(0, 1024, 8):
		var m := store.get_cell(wc - Vector3i(0, dy, 0))
		if m != 0:
			return [wc.y - dy, m]
	return [-1, 0]


## Debug: read the GPU volume back and compare with the store for cells inside the window.
func verify_upload(samples: int) -> Dictionary:
	var rng := RandomNumberGenerator.new()
	rng.seed = 11
	var bad := 0
	var first := []
	var layers := {}
	for i in samples:
		var wc := win + Vector3i(rng.randi_range(0, n - 1), rng.randi_range(0, n - 1), rng.randi_range(0, n - 1))
		var layer := wc.z & (n - 1)
		if not layers.has(layer):
			layers[layer] = _rd.texture_get_data(_tex_rid, layer)
		var data: PackedByteArray = layers[layer]
		var tex_v := data[(wc.x & (n - 1)) + n * (wc.y & (n - 1))]
		var store_v := store.get_cell(wc)
		if tex_v != store_v:
			bad += 1
			if first.size() < 5:
				first.append([wc.x, wc.y, wc.z, tex_v, store_v])
	return {"verify_samples": samples, "verify_bad": bad, "verify_first": first, "win": [win.x, win.y, win.z]}
