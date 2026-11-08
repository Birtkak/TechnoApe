class_name StainMap
extends Node

## Noita-style surface stains: blood, oil and soot coat the ground and walls
## they touch, and the coating stays after the liquid has run off, soaked in or
## burnt away. Water washes stains off again.
##
## Presentation only. A stain belongs to an empty cell (where the liquid or
## flame was) and colours the faces that border it (terrain_common.gdshaderinc:
## floors fully, walls up to a tide line, ceilings faintly). Sources:
##  - the simulation: after every tick SimBubble reports the chunks it changed;
##    each one in range is read once (one native copy) and its stain materials
##    are found with native PackedByteArray.count/find, so GDScript only
##    touches the hits (CLAUDE.md rule 8),
##  - bleed(): liquid blood from a hurt body that runs and soaks in.
## Storage: a sparse map world chunk -> 16 x 256 RGBA8 image (pixel (x, y*16+z),
## rgb = colour * amount, a = amount). The GPU sees a window around the player
## in a Texture2DArray (layer = world y, texel = world x, z; all wrapped), and
## changed layers are uploaded a few per frame.
## Stains are not saved with the world yet.

const CS := 16
const WIN_CHUNKS := Vector3i(8, 4, 8)          # window: 64 m x 32 m x 64 m around the player
const SIZE := WIN_CHUNKS * CS
const MAX_CHUNKS := 1024                       # 16 KB each: 16 MB at most; the farthest go first

# material id -> [colour (linear albedo), amount added per sighting]
var sources := {
	GameMaterials.BLOOD: [Color(0.13, 0.002, 0.007), 0.45],
	GameMaterials.OIL: [Color(0.045, 0.035, 0.022), 0.35],
	GameMaterials.FIRE: [Color(0.035, 0.032, 0.03), 0.06],   # soot
	GameMaterials.LAVA: [Color(0.05, 0.04, 0.035), 0.30],    # scorch
	GameMaterials.TOXIC_SLUDGE: [Color(0.09, 0.32, 0.02), 0.30],   # green slime trail
}
var washers := [GameMaterials.WATER, GameMaterials.WATER_STATIC]
var wash_rate := 0.3
var scan_budget_ms := 0.4      # main-thread time per sim tick for reading changed chunks
var max_hits_per_material := 96
var uploads_per_frame := 4     # 64 KB layers per frame

var world: World
var bubble: SimBubble
var materials: Array[ShaderMaterial] = []

var _chunks := {}              # world chunk -> Image
var _touched := {}             # world chunk -> bitmask of local y rows changed since the last flush
var _layers: Array[Image] = []
var _tex: Texture2DArray
var _dirty_layers := {}        # layer -> true, waiting for upload
var _slot_owner := {}          # window slot -> world chunk whose data the layer images hold there
var _win := Vector3i(1 << 20, 0, 0)   # world chunk of the window's min corner
var _valid_min := Vector3i.ZERO
var _valid_max := Vector3i.ZERO
var _queue := {}               # world chunk -> true: changed by the simulation, not read yet
var _empty_mat := PackedByteArray()   # material id -> 1 if bodies and liquids pass (stains live there)
var _solid_mat := PackedByteArray()   # material id -> 1 if it is ground a stain can sit on
var _focus := Vector3.ZERO
var _test := 0                 # stain_test=1: splatter, oil and blood near the player once loaded
var _test_at := -1.0

# stats
var deposits := 0
var washes := 0
var chunks_read := 0
var scan_ms_max := 0.0
var uploads := 0


func setup(p_world: World, p_bubble: SimBubble, p_materials: Array[ShaderMaterial]) -> void:
	world = p_world
	bubble = p_bubble
	var defs := GameMaterials.defs()
	_empty_mat.resize(defs.size() + 1)
	_solid_mat.resize(defs.size() + 1)
	_empty_mat[0] = 1
	for i in defs.size():
		var d: Dictionary = defs[i]
		var ct: String = d.get("cell_type", "solid")
		var solid: bool = (ct == "solid" or ct == "powder") and not d.has("micro") and d.get("model", "") != "empty"
		_solid_mat[i + 1] = 1 if solid else 0
		_empty_mat[i + 1] = 1 if (not solid and not d.has("micro") and ct != "solid") else 0
	for a in OS.get_cmdline_user_args():
		if a.begins_with("stain_test="):
			_test = int(a.get_slice("=", 1))
		elif a.begins_with("stain_uploads="):
			uploads_per_frame = int(a.get_slice("=", 1))
		elif a == "stains=0":
			return   # A/B runs: no stains at all (the shader's stains_on stays false)
	materials = p_materials
	for y in SIZE.y:
		_layers.append(Image.create(SIZE.x, SIZE.z, false, Image.FORMAT_RGBA8))
	_tex = Texture2DArray.new()
	_tex.create_from_images(_layers)
	for m in materials:
		m.set_shader_parameter("stain_tex", _tex)
		m.set_shader_parameter("stain_size", SIZE)
		m.set_shader_parameter("stains_on", true)
	if not OS.get_cmdline_user_args().has("stain_scan=0"):   # debug: stains only from bleed()
		bubble.chunks_changed.connect(_on_chunks_changed)


## Keeps the GPU window centred on `pos` (world voxels); call every frame.
func follow(pos: Vector3) -> void:
	_focus = pos
	if _tex == null:
		return
	var want := Vector3i((pos / CS).floor()) - WIN_CHUNKS / 2
	if want != _win:
		_move_window(want)


func _process(_delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(_delta)
	DebugCapture.note_proc("stains", _pt0)


func _process_timed(_delta: float) -> void:
	if _test > 0:
		_run_test()   # also with stains=0: the same spill without stains, for A/B runs
	if _tex == null:
		return
	_scan_queue()
	_flush_touched()
	var n := 0
	for l in _dirty_layers.keys():
		if n >= uploads_per_frame:
			break
		_tex.update_layer(_layers[l], l)
		_dirty_layers.erase(l)
		n += 1
		uploads += 1
	if _dirty_layers.is_empty() and (_valid_min != _win * CS or _valid_max != (_win + WIN_CHUNKS) * CS):
		_set_valid(_win * CS, (_win + WIN_CHUNKS) * CS)


# ---------------- sources ----------------

## Adds `amount` (0..1) of `colour` to the stain of the empty cell `wp` (world voxel).
func deposit(wp: Vector3i, colour: Color, amount: float) -> void:
	var wc := _chunk_of(wp)
	var img: Image = _chunks.get(wc)
	if img == null:
		img = Image.create(CS, CS * CS, false, Image.FORMAT_RGBA8)
		_chunks[wc] = img
		if _chunks.size() > MAX_CHUNKS:
			_evict_farthest()
	var l := wp - wc * CS
	var px := Vector2i(l.x, l.y * CS + l.z)
	var cur := img.get_pixelv(px)
	var a := cur.a
	var na := minf(1.0, a + amount)
	var prev := Color(cur.r / a, cur.g / a, cur.b / a) if a > 0.004 else colour
	var col := prev.lerp(colour, amount / (a + amount))
	img.set_pixelv(px, Color(col.r * na, col.g * na, col.b * na, na))
	_touched[wc] = int(_touched.get(wc, 0)) | (1 << l.y)
	deposits += 1


## Removes up to `amount` of the stain at `wp`.
func wash(wp: Vector3i, amount: float) -> void:
	var wc := _chunk_of(wp)
	var img: Image = _chunks.get(wc)
	if img == null:
		return
	var l := wp - wc * CS
	var px := Vector2i(l.x, l.y * CS + l.z)
	var cur := img.get_pixelv(px)
	if cur.a <= 0.0:
		return
	var na := maxf(0.0, cur.a - amount)
	var k := na / cur.a
	img.set_pixelv(px, Color(cur.r * k, cur.g * k, cur.b * k, na))
	_touched[wc] = int(_touched.get(wc, 0)) | (1 << l.y)
	washes += 1


## The stain amount (0..1) at a world voxel.
func amount_at(wp: Vector3i) -> float:
	var wc := _chunk_of(wp)
	var img: Image = _chunks.get(wc)
	if img == null:
		return 0.0
	var l := wp - wc * CS
	return img.get_pixel(l.x, l.y * CS + l.z).a


## A stained body drips: a small stain on the floor at or just below `at` (the feet).
func drip(at: Vector3, colour: Color, amount: float = 0.18) -> void:
	var p := Vector3i((at + Vector3(randf_range(-0.3, 0.3), 0.2, randf_range(-0.3, 0.3))).floor())
	var floor_cell := _floor_below(p, 2)
	if floor_cell.y > -1000:
		deposit(floor_cell, colour, amount)


## A placed block fills the cell: its old stain must not reappear when it is mined.
func clear_cell(wp: Vector3i) -> void:
	wash(wp, 1.0)


## Bleeding: `hp_lost` half-hearts. About 20 cells of liquid blood (more for heavy hits) burst
## from the body around `at` (world voxels, the feet), fall, run and soak in, staining where they
## go (GameMaterials.BLOOD has a lifetime). Real blood, no painted splatter (bug report 2026-10-28_08-35-43).
## Returns how many blood cells were placed (none outside the simulated bubble).
func bleed(at: Vector3, hp_lost: int) -> int:
	if hp_lost <= 0 or bubble == null:
		return 0
	var n := mini(6 + 2 * hp_lost, 20)   # was 16 + 4 per hp, cap 40: too much blood
	var r := Settings.m2v(0.3)
	var used := {}
	for i in n * 3:   # full or already picked cells are skipped, so try more
		if used.size() >= n:
			break
		var p := Vector3i((at + Vector3(randf_range(-r, r), randf_range(Settings.m2v(0.2), Settings.m2v(1.0)), randf_range(-r, r))).floor())
		if not used.has(p) and _is_empty(_mat(p)) and bubble.set_voxel(p, GameMaterials.BLOOD):
			used[p] = true
	return used.size()


## The empty cell sitting on solid ground at or below `p` (within `depth`
## cells), or y = -1000 when there is none.
func _floor_below(p: Vector3i, depth: int) -> Vector3i:
	for k in depth + 1:
		var q := p - Vector3i(0, k, 0)
		if _is_empty(_mat(q)) and _is_solid(_mat(q - Vector3i(0, 1, 0))):
			return q
	return Vector3i(0, -1000, 0)


func _mat(p: Vector3i) -> int:
	return world.get_voxel(p) if world != null else 0


func _is_empty(m: int) -> bool:
	return m >= 0 and m < _empty_mat.size() and _empty_mat[m] == 1


func _is_solid(m: int) -> bool:
	return m > 0 and m < _solid_mat.size() and _solid_mat[m] == 1


# ---------------- simulation feed ----------------

## SimBubble, right after a tick: the chunks it changed. Only queued here: the
## bubble starts its next tick after this returns, and reading now would delay it.
func _on_chunks_changed(world_chunks: Array) -> void:
	var lo := _win
	var hi := _win + WIN_CHUNKS
	for wc: Vector3i in world_chunks:
		if wc.x >= lo.x and wc.y >= lo.y and wc.z >= lo.z and wc.x < hi.x and wc.y < hi.y and wc.z < hi.z:
			_queue[wc] = true


## Reads queued chunks until the budget is spent, only while no tick runs (a read
## would wait for it); the rest wait for a later frame.
func _scan_queue() -> void:
	if _queue.is_empty() or not bubble.is_idle():
		return
	var t0 := Time.get_ticks_usec()
	var budget_us := int(scan_budget_ms * 1000.0)
	var done: Array[Vector3i] = []
	for wc: Vector3i in _queue:
		if not done.is_empty() and Time.get_ticks_usec() - t0 >= budget_us:
			break
		done.append(wc)
		_scan_chunk(wc)
	for wc in done:
		_queue.erase(wc)
	scan_ms_max = maxf(scan_ms_max, (Time.get_ticks_usec() - t0) / 1000.0)


func _scan_chunk(wc: Vector3i) -> void:
	var lc: Vector3i = wc - bubble.origin
	var sc := bubble.size_chunks
	if lc.x < 0 or lc.y < 0 or lc.z < 0 or lc.x >= sc.x or lc.y >= sc.y or lc.z >= sc.z:
		return
	if not bubble.is_chunk_loaded(lc):
		return
	var bytes: PackedByteArray = bubble.sim.get_chunk_bytes(lc, 1)   # index = y + 16 * (x + 16 * z)
	chunks_read += 1
	var base := wc * CS
	for m: int in sources:
		if bytes.count(m) == 0:
			continue
		var src: Array = sources[m]
		var i := bytes.find(m)
		var k := 0
		while i >= 0 and k < max_hits_per_material:
			deposit(base + Vector3i((i >> 4) & 15, i & 15, i >> 8), src[0], src[1])
			i = bytes.find(m, i + 1)
			k += 1
	if _chunks.has(wc):
		for m: int in washers:
			var i := bytes.find(m)
			var k := 0
			while i >= 0 and k < 128:
				var wp := base + Vector3i((i >> 4) & 15, i & 15, i >> 8)
				if amount_at(wp) > 0.0:
					wash(wp, wash_rate)
				i = bytes.find(m, i + 1)
				k += 1


# ---------------- GPU window ----------------

func _chunk_of(wp: Vector3i) -> Vector3i:
	return Vector3i(floori(wp.x / float(CS)), floori(wp.y / float(CS)), floori(wp.z / float(CS)))


func _slot_of(wc: Vector3i) -> Vector3i:
	return Vector3i(posmod(wc.x, WIN_CHUNKS.x), posmod(wc.y, WIN_CHUNKS.y), posmod(wc.z, WIN_CHUNKS.z))


func _in_window(wc: Vector3i) -> bool:
	var r := wc - _win
	return r.x >= 0 and r.y >= 0 and r.z >= 0 and r.x < WIN_CHUNKS.x and r.y < WIN_CHUNKS.y and r.z < WIN_CHUNKS.z


## Copies the chunk's changed rows (or all of them) into the layer images and
## marks those layers for upload. Chunks without stains clear their slot.
func _write_slot(wc: Vector3i, rows: int) -> void:
	var slot := _slot_of(wc)
	var img: Image = _chunks.get(wc)
	var dst := Vector2i(slot.x * CS, slot.z * CS)
	if img == null:
		if not _slot_owner.has(slot):
			return   # the slot is already clean
		for y in CS:
			var layer := slot.y * CS + y
			_layers[layer].fill_rect(Rect2i(dst, Vector2i(CS, CS)), Color(0, 0, 0, 0))
			_dirty_layers[layer] = true
		_slot_owner.erase(slot)
		return
	_slot_owner[slot] = wc
	for y in CS:
		if rows & (1 << y) == 0:
			continue
		var layer := slot.y * CS + y
		_layers[layer].blit_rect(img, Rect2i(0, y * CS, CS, CS), dst)
		_dirty_layers[layer] = true


func _flush_touched() -> void:
	for wc: Vector3i in _touched:
		if _in_window(wc):
			var slot := _slot_of(wc)
			# a slot still holding another chunk's stains gets all rows
			var rows: int = _touched[wc] if _slot_owner.get(slot, wc) == wc else 0xFFFF
			_write_slot(wc, rows)
	_touched.clear()


## The window moved: rewrite the slots of chunks that entered it. Until those
## layers are uploaded, the shader only trusts the part both windows share.
func _move_window(want: Vector3i) -> void:
	var old := _win
	_win = want
	var any := false
	for z in WIN_CHUNKS.z:
		for y in WIN_CHUNKS.y:
			for x in WIN_CHUNKS.x:
				var wc := want + Vector3i(x, y, z)
				var r := wc - old
				var was_in := r.x >= 0 and r.y >= 0 and r.z >= 0 and r.x < WIN_CHUNKS.x and r.y < WIN_CHUNKS.y and r.z < WIN_CHUNKS.z
				if was_in:
					continue
				if _chunks.has(wc) or _slot_owner.has(_slot_of(wc)):
					_write_slot(wc, 0xFFFF)
					any = true
	var new_min := want * CS
	var new_max := (want + WIN_CHUNKS) * CS
	if any or not _dirty_layers.is_empty():
		var omin := old * CS
		var omax := (old + WIN_CHUNKS) * CS
		var lo := Vector3i(maxi(omin.x, new_min.x), maxi(omin.y, new_min.y), maxi(omin.z, new_min.z))
		var hi := Vector3i(mini(omax.x, new_max.x), mini(omax.y, new_max.y), mini(omax.z, new_max.z))
		_set_valid(lo, Vector3i(maxi(lo.x, hi.x), maxi(lo.y, hi.y), maxi(lo.z, hi.z)))
	else:
		_set_valid(new_min, new_max)


func _set_valid(lo: Vector3i, hi: Vector3i) -> void:
	_valid_min = lo
	_valid_max = hi
	for m in materials:
		m.set_shader_parameter("stain_min", lo)
		m.set_shader_parameter("stain_max", hi)


func _evict_farthest() -> void:
	var c := Vector3i((_focus / CS).floor())
	var far := Vector3i.ZERO
	var best := -1
	for wc: Vector3i in _chunks:
		var d := (wc - c).length_squared()
		if d > best:
			best = d
			far = wc
	_chunks.erase(far)
	if _in_window(far):
		_write_slot(far, 0xFFFF)


# ---------------- debug ----------------

## stain_test=1 on the command line: 4 s after the simulation has loaded, a
## heavy bleed at the player, an oil blob and a blood blob dropped next to them.
func _run_test() -> void:
	if bubble == null or not bubble.is_loaded():
		return
	var now := Time.get_ticks_msec() / 1000.0
	if _test_at < 0.0:
		_test_at = now + 4.0
		return
	if now < _test_at:
		return
	_test = 0
	var feet := Vector3i(_focus.floor())
	bleed(_focus, 8)
	var drops := {GameMaterials.OIL: Vector3i(5, 3, 0), GameMaterials.BLOOD: Vector3i(-4, 3, 4)}
	for m: int in drops:
		var c: Vector3i = feet + drops[m]
		for z in range(-1, 2):
			for x in range(-1, 2):
				for y in 2:
					var p := c + Vector3i(x, y, z)
					if _is_empty(_mat(p)):
						bubble.set_voxel(p, m)
	print("stain_test: bled and dropped oil + blood around %s" % feet)


func stats() -> Dictionary:
	return {"chunks": _chunks.size(), "deposits": deposits, "washes": washes, "chunks_read": chunks_read,
		"queue": _queue.size(), "scan_ms_max": snappedf(scan_ms_max, 0.01), "uploads": uploads,
		"layers_pending": _dirty_layers.size()}
