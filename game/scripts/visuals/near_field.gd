class_name NearField
extends Node3D

## Ray-cast near field (ADR 0012 step 3): the brick world around the player,
## drawn per pixel at 6.25 cm cells by near_field.gdshader on a box, composited
## with the meshed far field by depth. Presentation only: nothing here changes
## the world.
##
## Data flow: the simulation bubble is the store's writer (every chunk it loads
## and every cell a tick changes lands in a wrapped BrickStore, brick = voxel,
## micro vegetation expanded into its cell pattern); the native NearFieldFeed
## picks the window chunks to (re)upload within a byte budget per frame and
## packs them; a compute shader scatters them into a toroidal volume texture
## and writes brick occupancy. All RenderingDevice work runs on the render
## thread (ADR 0016) through RenderingServer.call_on_render_thread.
##
## The feed's VALID box (what has been uploaded) drives both the ray-cast box
## and the terrain shaders' mesh-hiding box, so a slide never shows stale cells.

const CELLS_PER_VOXEL := 8
const CHUNK_CELLS := 16
const MAX_CHUNKS_PER_UPLOAD := 4096   # upload buffer: 16 MB of cells per frame at most
const RING := 3                       # staging buffer sets rotated per upload: a frame never overwrites what the previous one reads

var world: World
var bubble: SimBubble
var camera: Camera3D
var store: BrickStore
var feed: NearFieldFeed
var n := 512
var _store_chunk_origin := Vector3i(1 << 30, 0, 0)
var _mesh: MeshInstance3D
var _mat: ShaderMaterial
var _spirv: RDShaderSPIRV
# RenderingDevice resources: created and used on the render thread only
var _rd: RenderingDevice
var _vol_rid: RID
var _occ_rid: RID
var _shader: RID
var _pipeline: RID
var _bytes_bufs: Array[RID] = []
var _chunks_bufs: Array[RID] = []
var _usets: Array[RID] = []
var _ring := 0
var _vol_tex: Texture2DArrayRD
var _occ_tex: Texture2DArrayRD
var _rt_ready := false      # set by the render thread once the resources exist
var _verify := false        # near_verify=1: read the volume back once and compare it with the store (debug JSON line)
var _verify_layers := {}    # layer -> bytes, filled on the render thread
var _verify_occ := {}
var _verify_ready := false
var _verify_done := false
# stats
var uploads := 0
var upload_mb := 0.0
var upload_mb_max_frame := 0.0
var pack_ms_max := 0.0
var _pack_ms := PackedFloat32Array()   # main-thread ms per frame spent collecting + packing (p50/p99 in stats)
var slides := 0


func setup(p_world: World, p_bubble: SimBubble, p_camera: Camera3D) -> void:
	world = p_world
	bubble = p_bubble
	camera = p_camera
	# the window must sit inside the bubble with a margin of a chunk: what the bubble has not loaded is air to us
	var bubble_voxels: int = bubble.size_chunks.x * Settings.CHUNK
	n = 512 if Settings.near_field_m >= 24.0 else 256
	while n / CELLS_PER_VOXEL + 2 * Settings.CHUNK > bubble_voxels and n > 256:
		n /= 2
	var store_bricks := 1
	while store_bricks < bubble_voxels:
		store_bricks *= 2
	store = BrickStore.new()
	store.configure(Vector3i.ONE * store_bricks)
	store.set_wrap_origin(bubble.origin * Settings.CHUNK)
	store.set_micro_patterns(micro_patterns())
	_store_chunk_origin = bubble.origin
	bubble.store = store
	bubble.store_origin_voxel = Vector3i.ZERO   # world brick coordinates: the store wraps them itself
	feed = NearFieldFeed.new()
	feed.configure(store, n, MAX_CHUNKS_PER_UPLOAD)

	_mat = ShaderMaterial.new()
	_mat.shader = load("res://assets/shaders/near_field.gdshader")
	for k in ["atlas", "atlas_tiles", "water_tile", "lava_tile", "fire_tile", "glow_tile", "grass_top_tile", "grass_side_tile"]:
		_mat.set_shader_parameter(k, world.atlas_material.get_shader_parameter(k))
	_mat.set_shader_parameter("n", n)
	_mat.set_shader_parameter("cell", 1.0 / CELLS_PER_VOXEL)
	_mat.set_shader_parameter("max_bricks", n / 8 * 3)
	_mat.set_shader_parameter("water_id", GameMaterials.WATER)
	_mat.set_shader_parameter("water_static_id", GameMaterials.WATER_STATIC)
	_mat.set_shader_parameter("paint_base", PAINT_BASE)
	if OS.get_cmdline_user_args().has("near_debug=1"):   # where does the near field draw? (magenta tint)
		_mat.set_shader_parameter("debug_tint", 0.5)
	_verify = OS.get_cmdline_user_args().has("near_verify=1")
	var tiles := [PackedInt32Array(), PackedInt32Array(), PackedInt32Array()]
	for face in 3:
		tiles[face].resize(64)
		tiles[face].fill(0)
	var defs := GameMaterials.defs()
	for i in mini(defs.size(), 63):
		for face in 3:
			tiles[face][i + 1] = int(defs[i]["tiles"][face])
	_mat.set_shader_parameter("tile_top", tiles[0])
	_mat.set_shader_parameter("tile_side", tiles[1])
	_mat.set_shader_parameter("tile_bottom", tiles[2])
	_mesh = MeshInstance3D.new()
	_mesh.name = "Volume"
	var box := BoxMesh.new()
	box.size = Vector3.ONE
	_mesh.mesh = box
	_mesh.material_override = _mat
	_mesh.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF   # the terrain meshes cast; we only receive
	_mesh.ignore_occlusion_culling = true
	_mesh.visible = false
	add_child(_mesh)

	var sf: RDShaderFile = load("res://assets/shaders/near_scatter.glsl")
	_spirv = sf.get_spirv()
	RenderingServer.call_on_render_thread(_rt_init)


## Per micro material: 512 bytes (x fastest, y, z), 0 where the cell pattern is
## empty, else PAINT_BASE + the cell's atlas tile (the MicroMesher paint - 1), so a
## reed's head, a flower's petals and a mushroom's stem keep their own tiles on
## both sides of the seam. Cells >= PAINT_BASE are presentation: the brick's
## material is not recoverable from them (its majority is air, as the far field
## treats micro bricks anyway).
const PAINT_BASE := 128


static func micro_patterns() -> Dictionary:
	var out := {}
	var defs := GameMaterials.defs()
	for i in defs.size():
		var d: Dictionary = defs[i]
		if not d.has("micro"):
			continue
		var pattern: PackedInt32Array = GameMaterials.micro_pattern(d["micro"], d["tiles"])
		var cells := PackedByteArray()
		cells.resize(pattern.size())
		for k in pattern.size():
			cells[k] = (PAINT_BASE + pattern[k] - 1) if pattern[k] != 0 else 0
		out[i + 1] = cells
	return out


func _rt_init() -> void:
	_rd = RenderingServer.get_rendering_device()
	var fmt := RDTextureFormat.new()
	fmt.texture_type = RenderingDevice.TEXTURE_TYPE_2D_ARRAY
	fmt.format = RenderingDevice.DATA_FORMAT_R8_UNORM
	fmt.width = n
	fmt.height = n
	fmt.array_layers = n
	fmt.mipmaps = 1
	fmt.usage_bits = RenderingDevice.TEXTURE_USAGE_SAMPLING_BIT | RenderingDevice.TEXTURE_USAGE_STORAGE_BIT | RenderingDevice.TEXTURE_USAGE_CAN_UPDATE_BIT | RenderingDevice.TEXTURE_USAGE_CAN_COPY_FROM_BIT
	_vol_rid = _rd.texture_create(fmt, RDTextureView.new(), [])
	var ofmt := RDTextureFormat.new()
	ofmt.texture_type = RenderingDevice.TEXTURE_TYPE_2D_ARRAY
	ofmt.format = RenderingDevice.DATA_FORMAT_R8_UNORM
	ofmt.width = n / 8
	ofmt.height = n / 8
	ofmt.array_layers = n / 8
	ofmt.mipmaps = 1
	ofmt.usage_bits = RenderingDevice.TEXTURE_USAGE_SAMPLING_BIT | RenderingDevice.TEXTURE_USAGE_STORAGE_BIT | RenderingDevice.TEXTURE_USAGE_CAN_COPY_FROM_BIT
	_occ_rid = _rd.texture_create(ofmt, RDTextureView.new(), [])
	_shader = _rd.shader_create_from_spirv(_spirv)
	_pipeline = _rd.compute_pipeline_create(_shader)
	for r in RING:
		var bytes_buf := _rd.storage_buffer_create(MAX_CHUNKS_PER_UPLOAD * CHUNK_CELLS * CHUNK_CELLS * CHUNK_CELLS)
		var chunks_buf := _rd.storage_buffer_create(MAX_CHUNKS_PER_UPLOAD * 16)
		var uniforms: Array[RDUniform] = []
		for i in 4:
			var u := RDUniform.new()
			u.binding = i
			u.uniform_type = RenderingDevice.UNIFORM_TYPE_IMAGE if i < 2 else RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
			u.add_id([_vol_rid, _occ_rid, bytes_buf, chunks_buf][i])
			uniforms.append(u)
		_bytes_bufs.append(bytes_buf)
		_chunks_bufs.append(chunks_buf)
		_usets.append(_rd.uniform_set_create(uniforms, _shader, 0))
	# The sampler wrappers are made here, on the render thread, right after the RD textures exist:
	# made from the main thread they intermittently bound a stale texture (the near field then drew
	# nothing while the meshes inside its box were hidden, and one RS texture leaked at exit).
	_vol_tex = Texture2DArrayRD.new()
	_vol_tex.texture_rd_rid = _vol_rid
	_occ_tex = Texture2DArrayRD.new()
	_occ_tex.texture_rd_rid = _occ_rid
	_mat.set_shader_parameter("vol", _vol_tex)
	_mat.set_shader_parameter("occ", _occ_tex)
	_rt_ready = true


## Render thread: scatter `count` chunks (bytes + world chunk coords) into the volume.
func _rt_upload(bytes: PackedByteArray, coords: PackedInt32Array, count: int) -> void:
	_ring = (_ring + 1) % RING
	_rd.buffer_update(_bytes_bufs[_ring], 0, bytes.size(), bytes)
	var cb := coords.to_byte_array()
	_rd.buffer_update(_chunks_bufs[_ring], 0, cb.size(), cb)
	var pc := PackedByteArray()
	pc.resize(16)
	pc.encode_s32(0, n)
	pc.encode_s32(4, count)
	var cl := _rd.compute_list_begin()
	_rd.compute_list_bind_compute_pipeline(cl, _pipeline)
	_rd.compute_list_bind_uniform_set(cl, _usets[_ring], 0)
	_rd.compute_list_set_push_constant(cl, pc, pc.size())
	_rd.compute_list_dispatch(cl, count, 1, 1)
	_rd.compute_list_end()


## Render thread (debug): read volume layers and occupancy layers back.
func _rt_readback(layers: Array, occ_layers: Array) -> void:
	for l in layers:
		_verify_layers[l] = _rd.texture_get_data(_vol_rid, l)
	for l in occ_layers:
		_verify_occ[l] = _rd.texture_get_data(_occ_rid, l)
	_verify_ready = true


## Debug (near_verify=1): once the window is fully uploaded, sample cells of the valid box
## in the GPU volume and compare with the store; prints one JSON line.
func _verify_step() -> void:
	if _verify_done:
		return
	if not _verify_ready:
		if feed.pending_planes() == 0 and feed.pending_chunks() == 0 and bubble.is_loaded() and uploads > 4 and _verify_layers.is_empty():
			var vmin: Vector3i = feed.get_valid_min()
			var layers := []
			var occ_layers := []
			for k in 6:
				layers.append((vmin.z + 8 + k * 37) & (n - 1))
				occ_layers.append(((vmin.z >> 3) + k * 5) & ((n >> 3) - 1))
			RenderingServer.call_on_render_thread(_rt_readback.bind(layers, occ_layers))
		return
	_verify_done = true
	var vmin: Vector3i = feed.get_valid_min()
	var vmax: Vector3i = feed.get_valid_max()
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	var checked := 0
	var bad := 0
	var bad_occ := 0
	var first := []
	var solid_store := 0
	var solid_tex := 0
	var per_chunk := {}   # chunk -> [checked, bad]
	for layer in _verify_layers:
		var data: PackedByteArray = _verify_layers[layer]
		if data.size() != n * n:
			first.append({"layer": layer, "bytes": data.size()})
			continue
		for i in 3000:
			var wc := Vector3i(rng.randi_range(vmin.x, vmax.x - 1), rng.randi_range(vmin.y, vmax.y - 1), 0)
			# the layer holds world cells with z & (n-1) == layer: pick the z inside the box
			var z0 := (vmin.z & ~(n - 1)) | int(layer)
			wc.z = z0 if z0 >= vmin.z else z0 + n
			if wc.z >= vmax.z:
				continue
			var tex_v := data[(wc.x & (n - 1)) + n * (wc.y & (n - 1))]
			var store_v := store.get_cell(wc)
			checked += 1
			solid_store += 1 if store_v != 0 else 0
			solid_tex += 1 if tex_v != 0 else 0
			var ck := Vector3i(wc.x >> 4, wc.y >> 4, wc.z >> 4)
			if not per_chunk.has(ck):
				per_chunk[ck] = [0, 0]
			per_chunk[ck][0] += 1
			if tex_v != store_v:
				bad += 1
				per_chunk[ck][1] += 1
				if first.size() < 6:
					first.append({"cell": wc, "tex": tex_v, "store": store_v})
	for layer in _verify_occ:
		var data: PackedByteArray = _verify_occ[layer]
		var nb := n >> 3
		if data.size() != nb * nb:
			continue
		for i in 200:
			var b := Vector3i(rng.randi_range(vmin.x >> 3, (vmax.x >> 3) - 1), rng.randi_range(vmin.y >> 3, (vmax.y >> 3) - 1), 0)
			var z0 := ((vmin.z >> 3) & ~(nb - 1)) | int(layer)
			b.z = z0 if z0 >= (vmin.z >> 3) else z0 + nb
			if b.z >= (vmax.z >> 3):
				continue
			var occ_v := data[(b.x & (nb - 1)) + nb * (b.y & (nb - 1))] != 0
			var any := false
			if store.is_expanded(b):
				for k in 64:   # a sample of the brick's cells
					if store.get_cell(b * 8 + Vector3i(k & 7, (k >> 3) & 7, (k * 5) & 7)) != 0:
						any = true
						break
			else:
				any = store.brick_material(b) != 0
			if occ_v != any and not (occ_v and store.is_expanded(b)):   # a sampled expanded brick may miss its cells
				bad_occ += 1
	# chunk-level view: chunks with every sample wrong are missing uploads, mixed ones are cell errors
	var chunks_all_bad := 0
	var chunks_mixed := 0
	var chunks_ok := 0
	var bad_examples := []
	for ck in per_chunk:
		var v: Array = per_chunk[ck]
		if v[1] == 0:
			chunks_ok += 1
		elif v[1] == v[0] and v[0] >= 3:
			chunks_all_bad += 1
			if bad_examples.size() < 8:
				bad_examples.append(ck)
		else:
			chunks_mixed += 1
	print(JSON.stringify({"near_verify": true, "n": n, "checked": checked, "bad": bad, "solid_store": solid_store, "solid_tex": solid_tex, "bad_occ": bad_occ, "first": first, "valid_min": vmin, "valid_max": vmax, "win": feed.get_window_origin(), "chunks_ok": chunks_ok, "chunks_all_bad": chunks_all_bad, "chunks_mixed": chunks_mixed, "all_bad_examples": bad_examples, "feed": feed.stats()}))


func _rt_free() -> void:
	for rid in _usets + [_pipeline, _shader] + _bytes_bufs + _chunks_bufs + [_vol_rid, _occ_rid]:
		if rid.is_valid():
			_rd.free_rid(rid)


func _exit_tree() -> void:
	if bubble != null and bubble.store == store:
		bubble.store = null
	if _vol_tex != null:
		_vol_tex.texture_rd_rid = RID()
		_occ_tex.texture_rd_rid = RID()
	if _rt_ready:
		RenderingServer.call_on_render_thread(_rt_free)
		RenderingServer.force_sync()   # the render thread must run it before the server finalizes (else RIDs leak)


func _process(_delta: float) -> void:
	if not _rt_ready or bubble == null or not bubble.is_loaded() and bubble.chunks_loaded == 0:
		return
	if bubble.origin != _store_chunk_origin:
		_store_chunk_origin = bubble.origin
		store.set_wrap_origin(bubble.origin * Settings.CHUNK)
	# the window follows the camera in whole chunks (1 m), inside the world's height
	var cam_cell := Vector3i((camera.global_position * CELLS_PER_VOXEL).floor())
	var want := Vector3i(((cam_cell.x - n / 2) >> 4) << 4, ((cam_cell.y - n / 2) >> 4) << 4, ((cam_cell.z - n / 2) >> 4) << 4)
	want.y = clampi(want.y, 0, Settings.WORLD_HEIGHT_VOXELS * CELLS_PER_VOXEL - n)
	if feed.set_window(want):
		slides += 1
	var t0 := Time.get_ticks_usec()
	feed.collect_dirty()
	var up: Dictionary = feed.pop_upload(int(Settings.near_upload_mb * 1048576.0))
	var count: int = up["count"]
	if count > 0:
		var bytes: PackedByteArray = up["bytes"]
		RenderingServer.call_on_render_thread(_rt_upload.bind(bytes, up["coords"], count))
		uploads += 1
		var mb := bytes.size() / 1048576.0
		upload_mb += mb
		upload_mb_max_frame = maxf(upload_mb_max_frame, mb)
	var pack := (Time.get_ticks_usec() - t0) / 1000.0
	pack_ms_max = maxf(pack_ms_max, pack)
	_pack_ms.append(pack)
	_apply_bounds()
	if _verify:
		_verify_step()


## The valid box: ray-cast volume, mesh-hiding box and the shader's window uniforms.
func _apply_bounds() -> void:
	var vmin: Vector3i = feed.get_valid_min()
	var vmax: Vector3i = feed.get_valid_max()
	if vmax.x <= vmin.x or vmax.y <= vmin.y or vmax.z <= vmin.z:
		_mesh.visible = false
		_set_hide_box(Vector3.ZERO, Vector3.ZERO)
		return
	var bmin := Vector3(vmin) / CELLS_PER_VOXEL
	var bmax := Vector3(vmax) / CELLS_PER_VOXEL
	_mesh.visible = true
	_mesh.position = (bmin + bmax) * 0.5
	_mesh.scale = bmax - bmin
	_mat.set_shader_parameter("win_origin", feed.get_window_origin())
	_mat.set_shader_parameter("valid_min", vmin)
	_mat.set_shader_parameter("valid_max", vmax)
	_set_hide_box(bmin, bmax)


func _set_hide_box(bmin: Vector3, bmax: Vector3) -> void:
	for m in [world.atlas_material, world.liquid_material]:
		m.set_shader_parameter("near_min", bmin)
		m.set_shader_parameter("near_max", bmax)


func stats() -> Dictionary:
	if feed == null:
		return {}
	var s: Dictionary = feed.stats()
	s["store"] = store.stats()
	s["gpu_ready"] = _rt_ready
	s["slides_node"] = slides
	s["uploads_node"] = uploads
	s["upload_mb_total"] = snappedf(upload_mb, 0.1)
	s["upload_mb_max_frame"] = snappedf(upload_mb_max_frame, 0.01)
	s["pack_ms_max"] = snappedf(pack_ms_max, 0.01)
	var sorted := _pack_ms.duplicate()
	sorted.sort()
	if sorted.size() > 0:
		s["pack_ms_p50"] = snappedf(sorted[sorted.size() / 2], 0.01)
		s["pack_ms_p99"] = snappedf(sorted[int(sorted.size() * 0.99)], 0.01)
		var total := 0.0
		for v in sorted:
			total += v
		s["pack_ms_mean"] = snappedf(total / sorted.size(), 0.001)
	s["valid_min"] = feed.get_valid_min()
	s["valid_max"] = feed.get_valid_max()
	s["vram_volume_mb"] = (n * n * n + (n / 8) * (n / 8) * (n / 8)) / 1048576.0
	return s
