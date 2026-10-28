class_name BrushTool
extends RefCounted

## The brush's preview and its native path (ADR 0065). The player keeps the box outline; this adds a
## see-through shell of the exact cells a round dig (orange) or a placement would touch; a placement's
## shell is the held material's own voxel pattern washed blue (brush_ghost.gdshader). Big
## creative brushes (above Brush.NATIVE_ABOVE) dig, place and paint through ShapeTool: the cells are
## rasterised, filtered by a material mask and written in C++, never one GDScript call per voxel
## (principle 8), CELLS_PER_FRAME at a time so a 4 m cube never costs one long frame.
##
## A brush's cells are kept relative to its box's low corner (`lo`), cached per shape, size, mode
## and face: moving the aim only moves `lo`. ShapeTool.fill takes the cells against a window origin,
## so passing (window - lo) places the cached cells without a GDScript pass over them. The round
## big brushes (16 ms for a 4 m ball in ShapeTool.cells) are rasterised under the loading screen
## (warm_cache).

var preview: MeshInstance3D
var preview_mat: StandardMaterial3D
var ghost_mat: ShaderMaterial      # placements: the material's texture, blue and see-through
var bubble: SimBubble              # set by Main; null in tests: native ops then do nothing
var stats := {"preview_builds": 0, "preview_ms_max": 0.0, "native_digs": 0, "native_places": 0, "native_paints": 0,
	"native_voxels": 0, "native_ms_max": 0.0, "native_slice_ms_max": 0.0, "cells_builds": 0, "cells_ms_max": 0.0,
	"last_dig": {}, "last_place": {}}   # the newest dig / placement: aim, face normal, box, cells (run JSON, bug reports)
var _st := ShapeTool.new()
var _key := ""
var _jobs: Array[Dictionary] = []
var _cells := {}                   # cache key -> PackedInt32Array, cells relative to the brush box's lo
const CELLS_PER_FRAME := 6000       # cells a queued big brush writes per frame (~0.4 ms measured)


func _init() -> void:
	preview_mat = StandardMaterial3D.new()
	preview_mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	preview_mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	preview_mat.cull_mode = BaseMaterial3D.CULL_BACK   # outer faces only: from inside the shape (digging down in it) it is out of the way
	preview_mat.no_depth_test = false
	preview_mat.albedo_color = Color(1.0, 0.55, 0.25, 0.14)
	ghost_mat = ShaderMaterial.new()
	ghost_mat.shader = preload("res://assets/shaders/brush_ghost.gdshader")
	preview = MeshInstance3D.new()
	preview.name = "BrushPreview"
	preview.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	preview.visible = false


func _notification(what: int) -> void:
	if what == NOTIFICATION_PREDELETE and is_instance_valid(preview):
		if preview.is_inside_tree():
			preview.queue_free()
		else:
			preview.free()


# ---------------- cells ----------------

## Where a brush's box starts for this aim: its cells (rel_cells) are relative to it.
static func anchor(hit: Vector3i, front: Vector3i, size: int, shape: String, place: bool) -> Vector3i:
	if size > Brush.NATIVE_ABOVE or shape in Brush.BOX_SHAPES or size <= 1:
		return Brush.region(hit, front, size, shape, place)["lo"]
	var c := Brush._place_centre(hit, front, size) if place else Vector3(hit) + Vector3(0.5, 0.5, 0.5)
	return Vector3i((c - Vector3.ONE * size * 0.5).floor())


static func _cache_key(front_minus_hit: Vector3i, size: int, shape: String, place: bool) -> String:
	if size > Brush.NATIVE_ABOVE and shape in ["sphere", "cylinder"]:
		return "%s %d" % [shape, size]   # a round region's cells do not depend on the face
	return "%s %d %s %s" % [shape, size, place, front_minus_hit]


## The brush's cells relative to anchor(): GDScript cells for survival sizes, ShapeTool's for big
## ones, cached.
func rel_cells(hit: Vector3i, front: Vector3i, size: int, shape: String, place: bool) -> PackedInt32Array:
	var key := _cache_key(front - hit, size, shape, place)
	if _cells.has(key):
		return _cells[key]
	var t0 := Time.get_ticks_usec()
	var out := PackedInt32Array()
	if size > Brush.NATIVE_ABOVE:
		var r := Brush.region(hit, front, size, shape, place)
		out = _st.cells(r["params"], Vector3i.ZERO).get("cells", PackedInt32Array())
	else:
		var lo := anchor(hit, front, size, shape, place)
		var list: Array[Vector3i] = Brush.place_cells(hit, front, size, shape) if place else Brush.dig_cells(hit, size, shape, front - hit)
		out.resize(list.size() * 3)
		for i in list.size():
			var c := list[i] - lo
			out[i * 3] = c.x
			out[i * 3 + 1] = c.y
			out[i * 3 + 2] = c.z
	if _cells.size() > 64:
		for k in _cells.keys():
			if String(k).get_slice_count(" ") > 2:
				_cells.erase(k)   # keep the big round brushes warm_cache made (16-62 ms each to redo)
	_cells[key] = out
	stats["cells_builds"] += 1
	stats["cells_ms_max"] = maxf(stats["cells_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)
	return out


## World cells (x, y, z triples) of a brush: tests and small uses only (a GDScript pass).
func cells(hit: Vector3i, front: Vector3i, size: int, shape: String, place: bool) -> PackedInt32Array:
	var rel := rel_cells(hit, front, size, shape, place)
	var lo := anchor(hit, front, size, shape, place)
	var out := PackedInt32Array()
	out.resize(rel.size())
	for i in range(0, rel.size(), 3):
		out[i] = rel[i] + lo.x
		out[i + 1] = rel[i + 1] + lo.y
		out[i + 2] = rel[i + 2] + lo.z
	return out


## Rasterises the round big brushes now (under the loading screen), not on the first Ctrl + wheel.
func warm_cache() -> void:
	for shape in ["sphere", "cylinder"]:
		for size in Brush.CREATIVE_SIZES:
			if size > Brush.NATIVE_ABOVE:
				rel_cells(Vector3i.ZERO, Vector3i.UP, size, shape, false)


# ---------------- the preview ----------------

## The terrain's atlas for the build ghost (Player.setup_inventory).
func set_atlas(atlas_material: ShaderMaterial) -> void:
	for p in ["atlas", "atlas_tiles", "pattern_tiles", "water_tile", "lava_tile", "fire_tile", "far_fade_start", "far_fade_end"]:
		ghost_mat.set_shader_parameter(p, atlas_material.get_shader_parameter(p))


## Shows the shell for this aim; null hit hides it. A dig in a box shape shows none (the player's
## outline is enough); a placement of material `mat` shows that material's ghost, even one voxel. The box shapes (cube, flat, column) are one unit
## shell scaled to the box, exact at any size. Big round brushes show a half-resolution shell (the
## same shape at size / 2, drawn twice as large): their full meshes took up to 28 ms to build.
func show_at(hit: VoxelRaycastResult, size: int, shape: String, place: bool, mat: int = 0) -> void:
	if hit == null or (not place and (size <= 1 or shape in Brush.BOX_SHAPES)):
		preview.visible = false
		return
	var front: Vector3i = hit.previous_position
	var normal: Vector3i = front - hit.position
	var boxy := shape in Brush.BOX_SHAPES
	var key := "box" if boxy else "%s %d %s %s" % [shape, size, normal, place]
	var lo := anchor(hit.position, front, size, shape, place)
	var coarse := size > Brush.NATIVE_ABOVE and not boxy
	if key != _key:
		_key = key
		var t0 := Time.get_ticks_usec()
		var rel := PackedInt32Array([0, 0, 0]) if boxy else (rel_cells(Vector3i.ZERO, normal, size / 2, shape, place) if coarse else rel_cells(hit.position, front, size, shape, place))
		var hi := Vector3i.ZERO
		for i in range(0, rel.size(), 3):
			hi = Vector3i(maxi(hi.x, rel[i] + 1), maxi(hi.y, rel[i + 1] + 1), maxi(hi.z, rel[i + 2] + 1))
		var arr: Array = _st.mesh(rel, Vector3i.ZERO, hi)
		var m := ArrayMesh.new()
		if not arr.is_empty() and not (arr[Mesh.ARRAY_INDEX] as PackedInt32Array).is_empty():
			m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arr)
			m.surface_set_material(0, preview_mat)
		preview.mesh = m
		stats["preview_builds"] += 1
		stats["preview_ms_max"] = maxf(stats["preview_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)
	if boxy:
		preview.scale = Vector3(Brush.region(hit.position, front, size, shape, place)["size"])
	else:
		preview.scale = Vector3.ONE * (2.0 if coarse else 1.0)
	preview_mat.albedo_color = Color(0.35, 0.8, 1.0, 0.16) if place else Color(1.0, 0.55, 0.25, 0.2)
	preview.material_override = ghost_mat if place and mat > 0 else null
	if place and mat > 0:
		var t: Array = GameMaterials.defs()[mat - 1]["tiles"]
		ghost_mat.set_shader_parameter("tiles", Vector3(float(t[0]), float(t[1]), float(t[2])))
	preview_mat.no_depth_test = not place   # a dig lies inside the terrain (flush with the aimed face): the shell shows through
	if preview.is_inside_tree():
		var at := Vector3(lo)
		if coarse:
			# the half-size shape's box, doubled, centred where the full one is
			var full := Vector3(Brush.region(hit.position, front, size, shape, place)["size"])
			var half := Vector3(Brush.region(Vector3i.ZERO, normal, size / 2, shape, place)["size"]) * 2.0
			at += (full - half) * 0.5
		preview.global_position = at
	preview.visible = true


## Records a dig or a placement for the run's JSON and bug reports: where it was aimed and what box it covered.
func note(kind: String, aim: Vector3i, normal: Vector3i, box: AABB, cells: int) -> void:
	stats["last_" + kind] = {"aim": "%d %d %d" % [aim.x, aim.y, aim.z], "normal": "%d %d %d" % [normal.x, normal.y, normal.z],
		"lo": "%d %d %d" % [int(box.position.x), int(box.position.y), int(box.position.z)],
		"size": "%d %d %d" % [int(box.size.x), int(box.size.y), int(box.size.z)], "cells": cells}


## A one-voxel shell in the preview's vertex format, once per material (dig shell, build ghost), for MaterialWarmup.
func warm_mesh() -> Mesh:
	var arr: Array = _st.mesh(PackedInt32Array([0, 0, 0]), Vector3i.ZERO, Vector3i.ONE)
	var m := ArrayMesh.new()
	for i in 2:
		m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arr)
		m.surface_set_material(i, ghost_mat if i == 1 else preview_mat)
	return m


func hide() -> void:
	preview.visible = false


# ---------------- the native path ----------------

func _window() -> Vector3i:
	return bubble.origin * Settings.CHUNK


## Digs a big brush: every cell whose material has mask[m] != 0 becomes air. Queued: tick() writes
## it over the next frames. Big creative brushes drop nothing (world editing). Returns the cells queued.
func dig(hit: Vector3i, front: Vector3i, size: int, shape: String, mask: PackedByteArray, material_hint := 0) -> int:
	return _queue("dig", hit, front, size, shape, false, GameMaterials.AIR, mask, material_hint)


## Fills a big brush against a face with `material` where mask[m] != 0 (air, liquids, gases,
## replaceable plants). Queued like dig.
func place(hit: Vector3i, front: Vector3i, size: int, shape: String, material: int, mask: PackedByteArray) -> int:
	return _queue("place", hit, front, size, shape, true, material, mask, material)


## Paints the dig shape around `hit` (creative, Shift + right click): every cell with mask[m] != 0
## (plain solid voxels) takes `material`. Any size, queued like dig.
func paint(hit: Vector3i, front: Vector3i, size: int, shape: String, material: int, mask: PackedByteArray) -> int:
	return _queue("paint", hit, front, size, shape, false, material, mask, material)


## A fill is still being written (the player waits for it before the next big brush).
func busy() -> bool:
	return not _jobs.is_empty()


func _queue(kind: String, hit: Vector3i, front: Vector3i, size: int, shape: String, place: bool, material: int,
		mask: PackedByteArray, hint: int) -> int:
	var rel := rel_cells(hit, front, size, shape, place)
	if bubble == null or rel.is_empty():
		return 0
	var lo := anchor(hit, front, size, shape, place)
	var box := AABB(Vector3(lo), Vector3.ONE * (size + 1))
	if size > Brush.NATIVE_ABOVE or shape in Brush.BOX_SHAPES:
		box = AABB(Vector3(lo), Vector3(Brush.region(hit, front, size, shape, place)["size"]))
	_jobs.append({"kind": kind, "cells": rel, "lo": lo, "i": 0, "material": material, "mask": mask, "hint": hint, "box": box,
		"n": 0, "ms": 0.0})
	return rel.size() / 3


## Writes queued fills, at most CELLS_PER_FRAME cells this frame. Returns the jobs that finished:
## {kind, cells (relative), lo, material, hint, box, n (cells changed)}.
func tick() -> Array:
	var done := []
	var budget := CELLS_PER_FRAME
	while budget > 0 and not _jobs.is_empty() and bubble != null:
		var j: Dictionary = _jobs[0]
		var c: PackedInt32Array = j["cells"]
		var from: int = j["i"]
		var to := mini(from + budget, c.size() / 3)
		var t0 := Time.get_ticks_usec()
		# the window origin minus lo: ShapeTool reads the relative cells as world cells at lo
		SimBubble.note_player_edit()
		j["n"] += _st.fill(bubble.sim, c.slice(from * 3, to * 3), j["material"], PackedByteArray(), _window() - j["lo"], j["mask"])
		var ms := (Time.get_ticks_usec() - t0) / 1000.0
		stats["native_slice_ms_max"] = maxf(stats["native_slice_ms_max"], ms)
		j["ms"] += ms
		budget -= to - from
		j["i"] = to
		if to * 3 >= c.size():
			_jobs.pop_front()
			stats["native_" + str(j["kind"]) + "s"] += 1
			stats["native_voxels"] += j["n"]
			stats["native_ms_max"] = maxf(stats["native_ms_max"], j["ms"])
			done.append(j)
	return done


## Would the brush fill any cell inside `body` (the player)? Native: the cells' grid over the box.
func overlaps(hit: Vector3i, front: Vector3i, size: int, shape: String, place: bool, body: AABB) -> bool:
	var lo := anchor(hit, front, size, shape, place)
	var blo := Vector3i(body.position.floor()) - lo
	var s := Vector3i(body.end.ceil()) - lo - blo
	return _st.grid_of(rel_cells(hit, front, size, shape, place), 1, PackedByteArray(), blo, s).count(1) > 0


## Material masks (256 entries): what a brush may dig with `held` (creative: every plain voxel and
## plant), what a placement may fill, what paint may recolour.
static func dig_mask(creative: bool, held: Dictionary, mining: MiningTable, is_plant: Callable) -> PackedByteArray:
	var mask := PackedByteArray()
	mask.resize(256)
	for i in mini(GameMaterials.defs().size(), 255):
		var m := i + 1
		if GameMaterials.is_monolith(m) or not (GameMaterials.is_plain_voxel(m) or is_plant.call(m)):
			continue
		if not creative and mining and mining.break_time(GameMaterials.name_of(m), held) < 0.0:
			continue
		mask[m] = 1
	return mask


static func place_mask(placeable: Callable) -> PackedByteArray:
	var mask := PackedByteArray()
	mask.resize(256)
	for m in 256:
		mask[m] = 1 if placeable.call(m) else 0
	return mask


static func paint_mask() -> PackedByteArray:
	var mask := PackedByteArray()
	mask.resize(256)
	for i in mini(GameMaterials.defs().size(), 255):
		if GameMaterials.is_plain_voxel(i + 1) and not GameMaterials.is_monolith(i + 1):
			mask[i + 1] = 1
	return mask
