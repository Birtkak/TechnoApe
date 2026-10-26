class_name WorldMap
extends Node

## The explored map (plan item D10, ADR 0055).
## The world is cut into tiles of TILE x TILE map pixels, one pixel per PX x PX voxel columns
## (0.5 m), each an RGBA8 Image saved per world (user://worlds/map_<seed>_g<gen>/<tx>_<tz>.png):
## - A == 0: not explored;
## - A == 255: generated terrain only, RGB = WorldGen.map_image's colour (biome, shaded by height);
## - else the real surface the sim saw: R = material id, G = liquid depth in voxels, B | A << 8 = the
##   surface height in voxels (A is 4..20 for the world's heights, never 0 or 255).
## The map shader (assets/shaders/map_relief.gdshader) colours materials by their atlas tile and
## shades the relief; MapView draws the tiles, Minimap and MapScreen use MapView.
## Exploring fills CELL x CELL pixel cells within Settings.map_explore_m of the player, nearest
## first, one native call per cell and frame step: FineGen.map_image for new ground (on a worker
## thread at GEN_STEP voxels a sample: 10-40 ms a cell), and, once the sim has it
## (SimWorld.surface_map, part A), the real voxels of the live window, refreshed over and over so digs
## and builds show. GDScript never walks columns (principle 8).

const PX := 4            # voxels per map pixel (0.5 m)
const TILE := 256        # map pixels per tile side (128 m)
const CELL := 32         # map pixels per explore cell side (16 m)
const STEP_S := 0.05     # one cell job this often
const GEN_STEP := 8      # voxels a generated-terrain sample (1 m), drawn 2 x 2 map pixels

var main: Node
var waypoints := Waypoints.new()
var palette: ImageTexture        # 256 x 1: each material's average top-tile colour
var _tiles := {}                 # Vector2i -> Image
var _textures := {}              # Vector2i -> ImageTexture
var _tex_dirty := {}             # Vector2i -> true: upload before the next draw
var _save_dirty := {}            # Vector2i -> true: write on the next save
var _missing := {}               # Vector2i -> true: no file on disk (checked once)
var _step_t := 0.0
var _refresh_i := 0              # round robin over the live cells (real surface)
var _dir := ""
var _gen_task := -1              # WorkerThreadPool task making a generated cell, or -1
var _gen_cell := Vector2i.ZERO
var _gen_bytes := PackedByteArray()
var stats := {"cells_generated": 0, "cells_surface": 0, "tiles": 0, "job_ms_max": 0.0, "saved": 0}
signal tiles_changed


func setup(p_main: Node) -> void:
	main = p_main
	_dir = "user://worlds/map_%d_g%d" % [Settings.world_seed, TerrainGenerator.GEN_VERSION]
	palette = ImageTexture.create_from_image(build_palette())


## Each material's average colour of its top atlas tile (box-filtered down to one pixel).
static func build_palette() -> Image:
	var img := Image.create(256, 1, false, Image.FORMAT_RGBA8)
	var atlas: Image = (load("res://assets/textures/atlas.png") as Texture2D).get_image()
	if atlas.is_compressed():
		atlas.decompress()
	var cols: int = GameMaterials.ATLAS_TILES.x
	var ts := atlas.get_width() / cols
	var defs := GameMaterials.defs()
	for i in mini(defs.size(), 255):
		var t: int = int(defs[i].get("tiles", [0])[0])
		var r := atlas.get_region(Rect2i((t % cols) * ts, (t / cols) * ts, ts, ts))
		r.convert(Image.FORMAT_RGBA8)
		var s := ts
		while s > 1:
			s = maxi(1, s / 2)
			r.resize(s, s, Image.INTERPOLATE_BILINEAR)   # halving with bilinear = a 2x2 box average
		img.set_pixel(i + 1, 0, r.get_pixel(0, 0))
	return img


# ---------------- tiles ----------------

static func tile_of_px(px: Vector2i) -> Vector2i:
	return Vector2i(floori(float(px.x) / TILE), floori(float(px.y) / TILE))


## The map pixel of world voxel column (x, z).
static func px_of(x: float, z: float) -> Vector2i:
	return Vector2i(floori(x / PX), floori(z / PX))


func _path(t: Vector2i) -> String:
	return "%s/%d_%d.png" % [_dir, t.x, t.y]


## The tile's image, loaded from disk once, or null when nothing of it was explored.
func tile_image(t: Vector2i, create := false) -> Image:
	if _tiles.has(t):
		return _tiles[t]
	if not _missing.has(t) and Settings.persist_world and FileAccess.file_exists(_path(t)):
		var img := Image.load_from_file(ProjectSettings.globalize_path(_path(t)))
		if img != null and img.get_width() == TILE:
			img.convert(Image.FORMAT_RGBA8)
			_tiles[t] = img
			_tex_dirty[t] = true
			return img
	_missing[t] = true
	if not create:
		return null
	var n := Image.create(TILE, TILE, false, Image.FORMAT_RGBA8)
	_tiles[t] = n
	stats["tiles"] = _tiles.size()
	return n


## The tile's texture for drawing (uploaded when it changed), or null.
func tile_texture(t: Vector2i) -> Texture2D:
	var img := tile_image(t)
	if img == null:
		return null
	if not _textures.has(t):
		_textures[t] = ImageTexture.create_from_image(img)
		_tex_dirty.erase(t)
	elif _tex_dirty.has(t):
		(_textures[t] as ImageTexture).update(img)
		_tex_dirty.erase(t)
	return _textures[t]


## True when map pixel px has been explored.
func explored(px: Vector2i) -> bool:
	var img := tile_image(tile_of_px(px))
	if img == null:
		return false
	return img.get_pixel(posmod(px.x, TILE), posmod(px.y, TILE)).a8 != 0


func _write(cell_px: Vector2i, src: Image, masked: bool) -> void:
	var t := tile_of_px(cell_px)
	var img := tile_image(t, true)
	var at := Vector2i(posmod(cell_px.x, TILE), posmod(cell_px.y, TILE))
	if masked:
		img.blit_rect_mask(src, src, Rect2i(Vector2i.ZERO, src.get_size()), at)
	else:
		img.blit_rect(src, Rect2i(Vector2i.ZERO, src.get_size()), at)
	_tex_dirty[t] = true
	_save_dirty[t] = true
	tiles_changed.emit()


# ---------------- exploring ----------------

func _process(delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(delta)
	DebugCapture.note_proc("map", _pt0)


func _process_timed(delta: float) -> void:
	if main == null or main.player == null or main.world == null:
		return
	_step_t -= delta
	if _step_t > 0.0:
		return
	_step_t = STEP_S
	var t0 := Time.get_ticks_usec()
	if _gen_task >= 0:
		if not WorkerThreadPool.is_task_completed(_gen_task):
			return
		WorkerThreadPool.wait_for_task_completion(_gen_task)
		_gen_task = -1
		_generated_done()
	_job()
	stats["job_ms_max"] = maxf(stats["job_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)


## The cells (their corner map pixel) within the explore radius, nearest first.
func _cells_near() -> Array[Vector2i]:
	var p: Vector3 = main.player.position
	var c := px_of(p.x, p.z)
	var r := ceili(Settings.m2v(Settings.map_explore_m) / PX / CELL)
	var me := Vector2i(floori(float(c.x) / CELL), floori(float(c.y) / CELL))
	var out: Array[Vector2i] = []
	for dz in range(-r, r + 1):
		for dx in range(-r, r + 1):
			if dx * dx + dz * dz <= r * r:
				out.append((me + Vector2i(dx, dz)) * CELL)
	out.sort_custom(func(a: Vector2i, b: Vector2i) -> bool:
		return (a - me * CELL).length_squared() < (b - me * CELL).length_squared())
	return out


## One step: the nearest unexplored cell gets the generated terrain; with nothing new to explore,
## one cell inside the live sim window gets the real surface (round robin).
func _job() -> void:
	var cells := _cells_near()
	for c in cells:
		if _gen_task >= 0 and c == _gen_cell:
			continue
		if not explored(c + Vector2i(CELL / 2, CELL / 2)):
			if not _surface(c):
				_generated(c)
			return
	if cells.is_empty():
		return
	_refresh_i = (_refresh_i + 1) % cells.size()
	_surface(cells[_refresh_i])


## Generated terrain for one cell (FineGen.map_image: biome colour shaded by height), made on a
## worker thread; _generated_done writes it.
func _generated(c: Vector2i) -> void:
	var gen = main.world.generator.gen if main.world.generator else null
	if gen == null or _gen_task >= 0:
		return
	var half := CELL * PX / 2
	_gen_cell = c
	_gen_task = WorkerThreadPool.add_task(func() -> void:
		_gen_bytes = shade_generated(gen, c * PX, gen.map_image(c.x * PX + half, c.y * PX + half, half, GEN_STEP)), false, "map cell")


## Bakes the shaded relief into a generated cell's colours (worker thread): the generator's height every
## GEN_STEP voxels, slopes over two samples each way (the 1 m roughness would read as grain), lit from
## the north-west like map_relief.gdshader; water stays flat, and the generator's spring and waterfall
## markers (yellow, white) are drawn as water. (n + 4)^2 height_at calls, a few ms off the main thread.
static func shade_generated(gen, corner: Vector2i, rgb: PackedByteArray) -> PackedByteArray:
	var n := CELL * PX / GEN_STEP
	if rgb.size() != n * n * 3:
		return rgb
	var w := n + 4
	var h := PackedFloat32Array()
	h.resize(w * w)
	for j in w:
		for i in w:
			h[j * w + i] = float(gen.height_at(corner.x + (i - 2) * GEN_STEP, corner.y + (j - 2) * GEN_STEP))
	for j in n:
		for i in n:
			var k := (j * n + i) * 3
			if (rgb[k] == 255 and rgb[k + 1] == 255) and (rgb[k + 2] == 0 or rgb[k + 2] == 255):
				rgb[k] = 58
				rgb[k + 1] = 120
				rgb[k + 2] = 222
				continue   # a spring or a waterfall's top: water on our map
			if rgb[k + 2] > rgb[k] + 40 and rgb[k + 2] > rgb[k + 1]:
				continue   # water
			var c0 := (j + 2) * w + i + 2
			var sx := (h[c0 + 2] - h[c0 - 2]) / (4.0 * GEN_STEP)
			var sz := (h[c0 + 2 * w] - h[c0 - 2 * w]) / (4.0 * GEN_STEP)
			var shade := clampf(1.0 - 0.9 * (sx + sz), 0.55, 1.35)   # light from -x, -z
			for c in 3:
				rgb[k + c] = clampi(roundi(rgb[k + c] * shade), 0, 255)
	return rgb


func _generated_done() -> void:
	var n := CELL * PX / GEN_STEP
	if _gen_bytes.size() != n * n * 3:
		return
	var img := Image.create_from_data(n, n, false, Image.FORMAT_RGB8, _gen_bytes)
	img.resize(CELL, CELL, Image.INTERPOLATE_NEAREST)
	img.convert(Image.FORMAT_RGBA8)   # alpha 255: generated-terrain colour
	_write(_gen_cell, img, false)
	stats["cells_generated"] += 1


func _exit_tree() -> void:
	if _gen_task >= 0:
		WorkerThreadPool.wait_for_task_completion(_gen_task)   # never leave a task running at quit
		_gen_task = -1


## The real surface of one cell from the live sim window (SimWorld.surface_map, part A). False when
## the sim cannot give it (not built in yet, the cell is outside the window, or a tick is running).
func _surface(c: Vector2i) -> bool:
	var bubble: SimBubble = main.get("bubble")
	if bubble == null or bubble.sim == null or not bubble.sim.has_method("surface_map") or not bubble.can_sample():
		return false
	var o: Vector3i = bubble.origin * Settings.CHUNK
	var size: Vector3i = bubble.sim.get_size_voxels()
	var lo := Vector2i(c.x * PX - o.x, c.y * PX - o.z)
	if lo.x < 0 or lo.y < 0 or lo.x + CELL * PX > size.x or lo.y + CELL * PX > size.z:
		return false
	var ints: PackedInt32Array = bubble.sim.surface_map(lo, Vector2i(CELL, CELL), PX, o.y)
	if ints.size() != CELL * CELL:
		return false
	_write(c, Image.create_from_data(CELL, CELL, false, Image.FORMAT_RGBA8, ints.to_byte_array()), true)
	stats["cells_surface"] += 1
	return true


# ---------------- saving ----------------

## Writes the tiles that changed (Main calls it with the world's other saves).
func save() -> void:
	if not Settings.persist_world or _save_dirty.is_empty():
		return
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(_dir))
	for t in _save_dirty:
		(_tiles[t] as Image).save_png(_path(t))
		stats["saved"] += 1
	_save_dirty.clear()


func summary() -> Dictionary:
	var s := stats.duplicate()
	s["waypoints"] = waypoints.list.size()
	s["native_surface"] = main != null and main.get("bubble") != null and main.bubble.sim != null and main.bubble.sim.has_method("surface_map")
	return s
