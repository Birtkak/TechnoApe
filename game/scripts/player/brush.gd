class_name Brush
extends RefCounted

## The dig and build brush (ADR 0034, ADR 0065, ADR 0073): which 12.5 cm voxels one dig or one
## placement touches. Pure logic, headless testable.
##   cube      a size^3 cube
##   sphere    a ball of diameter `size` centred on the aimed voxel
##   cylinder  an upright cylinder, diameter and height `size`
##   flat      a size x size square in the aimed face's plane, one voxel deep (shaving a wall,
##             levelling a floor, laying a floor or a wall skin)
##   column    a size/2 wide square bar 2 x size long along the aimed face's normal: into the
##             material when digging (a shaft down, a tunnel in), out of the face when placing (a
##             pillar up, a beam out)
##   tunnel    a hallway the body walks (ADR 0091): Settings.tunnel_width_m x tunnel_height_m across a
##             wall face, its floor at the feet (tunnel_floor_y), size / 2 voxels deep per dig (a 1 m
##             brush digs 0.5 m: as many cells as the 1 m cube, the GDScript dig's budget), snapped to the block grid; on a floor or ceiling a shaft that wide. It digs
##             only; placing with it places the cube.
## Where it goes (ADR 0073, "placing was off centre"): the round shapes dig around the aimed voxel
## and sit on the aimed face when placing. The box shapes (cube, flat, column) are **flush with the
## aimed face**, into the material when digging and out of it when placing, and **centred on the
## aim across the face**, snapped there to the 0.5 m block grid (BLOCK voxels) from size BLOCK up so
## placed blocks tile without gaps, and to their own size below it. A 1 m cube dug from a floor is a
## 1 m hole 1 m deep under the crosshair; placed on it, a 1 m cube standing on the floor there.
## `normal` is the aimed face's direction (front - hit).
## Sizes up to 8 are survival's (MiningTable.dig_cap caps digging by tool tier); creative adds
## 16 and 32 (4 m), which go through the native ShapeTool (`region`) instead of GDScript cells.

const SHAPES := ["cube", "sphere", "cylinder", "flat", "column", "tunnel"]
const BOX_SHAPES := ["cube", "flat", "column", "tunnel"]
const SIZES := [1, 2, 4, 8]
const CREATIVE_SIZES := [1, 2, 4, 8, 16, 32]
const NATIVE_ABOVE := 8   # brushes bigger than this are filled natively (ShapeTool), never cell by cell

static var tunnel_floor_y := 0   # the voxel row the player stands in (Player sets it while on the ground)


## Sizes Ctrl + wheel steps through in a mode.
static func sizes_for(creative: bool) -> Array:
	return CREATIVE_SIZES if creative else SIZES


## Cells a dig at the aimed voxel `hit` touches.
static func dig_cells(hit: Vector3i, size: int, shape: String, normal := Vector3i.UP) -> Array[Vector3i]:
	if size <= 1 and shape != "tunnel":
		return [hit]
	if shape in BOX_SHAPES:
		var b := _dig_box(hit, size, shape, normal)
		return _box(b.position, b.size)
	return _round(Vector3(hit) + Vector3(0.5, 0.5, 0.5), size, shape)


## Cells a placement against the face of `hit` touches; `front` is the voxel
## in front of that face (VoxelRaycastResult.previous_position).
static func place_cells(hit: Vector3i, front: Vector3i, size: int, shape: String) -> Array[Vector3i]:
	if size <= 1:
		return [front]
	if shape in BOX_SHAPES:
		var b := _place_box(hit, front, size, shape)
		return _box(b.position, b.size)
	return _round(_place_centre(hit, front, size), size, shape)


## The world box (position, size in voxels) a dig covers: the highlight outline.
static func dig_bounds(hit: Vector3i, size: int, shape: String, normal := Vector3i.UP) -> AABB:
	if size <= 1 and shape != "tunnel":
		return AABB(Vector3(hit), Vector3.ONE)
	if shape in BOX_SHAPES:
		return _dig_box(hit, size, shape, normal)
	var c := Vector3(hit) + Vector3(0.5, 0.5, 0.5)
	return AABB(c - Vector3.ONE * size * 0.5, Vector3.ONE * size)


## The world box a placement covers: the highlight outline.
static func place_bounds(hit: Vector3i, front: Vector3i, size: int, shape: String) -> AABB:
	if size <= 1:
		return AABB(Vector3(front), Vector3.ONE)
	if shape in BOX_SHAPES:
		return _place_box(hit, front, size, shape)
	var c := _place_centre(hit, front, size)
	return AABB(c - Vector3.ONE * size * 0.5, Vector3.ONE * size)


## The box and ShapeTool params of a brush for the native path (big creative brushes and the
## preview): {lo: Vector3i, size: Vector3i, params: Dictionary for ShapeTool.cells(params, lo)}.
## Round shapes take an odd box (size + 1) centred on the voxel, like the GDScript cells.
static func region(hit: Vector3i, front: Vector3i, size: int, shape: String, place: bool) -> Dictionary:
	if shape in BOX_SHAPES or size <= 1:
		var b: AABB = AABB(Vector3(front if place else hit), Vector3.ONE)
		if size > 1 or (shape == "tunnel" and not place):
			b = _place_box(hit, front, size, shape) if place else _dig_box(hit, size, shape, front - hit)
		var s := Vector3i(b.size)
		return {"lo": Vector3i(b.position), "size": s, "params": {"shape": "box", "size": s}}
	var c := _place_centre(hit, front, size) if place else Vector3(hit) + Vector3(0.5, 0.5, 0.5)
	var n := size + 1
	var lo := Vector3i((c - Vector3.ONE * n * 0.5).round())
	var s := Vector3i.ONE * n
	return {"lo": lo, "size": s, "params": {"shape": shape, "size": s}}


## The column's cross-section and length.
static func column_dims(size: int) -> Vector2i:
	return Vector2i(maxi(1, size / 2), 2 * size)


## The tunnel's width and height in voxels.
static func tunnel_dims() -> Vector2i:
	return Vector2i(roundi(Settings.m2v(Settings.tunnel_width_m)), roundi(Settings.m2v(Settings.tunnel_height_m)))


## How deep one tunnel dig goes.
static func tunnel_depth(size: int) -> int:
	return maxi(1, size / 2)


## A short description for the HUD and toasts: "sphere 1 m", "flat 1 x 1 m", "column 0.5 x 2 m".
static func describe(size: int, shape: String) -> String:
	match shape:
		"flat":
			return "flat %s square" % _len(size)
		"column":
			var d := column_dims(size)
			return "column %s wide, %s long" % [_len(d.x), _len(d.y)]
		"tunnel":
			return "tunnel %s x %s, %s a dig" % [_len(tunnel_dims().x), _len(tunnel_dims().y), _len(tunnel_depth(size))]
	return "%s %s" % [shape, _len(size)]


## What a dig and a placement of this brush do, in words, for the tools panel and the HUD
## ("dig: a 1 m cube into the aimed face", "place: a 1 m cube against it").
static func explain(size: int, shape: String, place: bool) -> String:
	if size <= 1 and (place or shape != "tunnel"):
		return "the voxel in front of the aimed face" if place else "the aimed voxel"
	match shape:
		"cube":
			return "a %s cube %s" % [_len(size), "standing on the aimed face" if place else "into the aimed face"]
		"sphere":
			return "a %s ball %s" % [_len(size), "resting on the aimed face" if place else "around the aimed voxel"]
		"cylinder":
			return "an upright %s cylinder %s" % [_len(size), "resting on the aimed face" if place else "around the aimed voxel"]
		"flat":
			return "a %s square one voxel thick %s" % [_len(size), "laid on the aimed face" if place else "shaved off the aimed face"]
		"column":
			var d := column_dims(size)
			return "a %s wide, %s long bar %s" % [_len(d.x), _len(d.y), "out of the aimed face" if place else "into the aimed face"]
		"tunnel":
			if place:
				return explain(size, "cube", true)
			return "a %s wide, %s tall hallway %s into the aimed wall, floor at your feet" % [_len(tunnel_dims().x), _len(tunnel_dims().y), _len(tunnel_depth(size))]
	return describe(size, shape)


static func _len(v: int) -> String:
	var m := v * Settings.VOXEL_M
	return ("%d cm" % roundi(m * 100.0)) if m < 1.0 else ("%s m" % str(snappedf(m, 0.25)))


static func _axis(normal: Vector3i) -> int:
	var a := normal.abs()
	if a.x >= a.y and a.x >= a.z and a.x > 0:
		return 0
	if a.z >= a.y and a.z > 0:
		return 2
	return 1


## The low corner across the face: `extent` cells centred on voxel `c`, snapped to the grid blocks
## tile on (BLOCK voxels from extent BLOCK up, the extent itself below).
static func _centre_lo(c: int, extent: int) -> int:
	var g := Settings.BLOCK if extent >= Settings.BLOCK else extent
	return floori((float(c) + 0.5 - extent * 0.5) / g + 0.5) * g


## The box a dig or a placement covers for the box shapes: `across` cells centred on the aim across
## the face, `depth` cells along its normal, into the material (dig) or out of the face (place). A
## zero normal (the ray started inside the voxel) counts as a top face.
static func _face_box(hit: Vector3i, normal: Vector3i, across: int, depth: int, place: bool) -> AABB:
	var ax := _axis(normal)
	var sgn := -1 if normal[ax] < 0 else 1
	var lo := Vector3i(_centre_lo(hit.x, across), _centre_lo(hit.y, across), _centre_lo(hit.z, across))
	var s := Vector3i.ONE * across
	s[ax] = depth
	if place:
		var front := hit[ax] + sgn
		lo[ax] = front if sgn > 0 else front - depth + 1
	else:
		lo[ax] = hit[ax] - depth + 1 if sgn > 0 else hit[ax]
	return AABB(Vector3(lo), Vector3(s))


static func _dig_box(hit: Vector3i, size: int, shape: String, normal: Vector3i) -> AABB:
	match shape:
		"flat":
			return _face_box(hit, normal, size, 1, false)
		"column":
			var d := column_dims(size)
			return _face_box(hit, normal, d.x, d.y, false)
		"tunnel":
			return _tunnel_box(hit, tunnel_depth(size), normal)
	return _face_box(hit, normal, size, size, false)


## The tunnel's dig box: across a wall face the hallway from the feet up, its slice along the normal
## on the block grid once it is a block deep, so step after step lines up; on a floor or ceiling a shaft.
static func _tunnel_box(hit: Vector3i, depth: int, normal: Vector3i) -> AABB:
	var w := tunnel_dims().x
	var ax := _axis(normal)
	if ax == 1:
		return _face_box(hit, normal, w, depth, false)
	var b := _face_box(hit, normal, w, depth, false)
	var lo := Vector3i(b.position)
	var s := Vector3i(b.size)
	lo.y = tunnel_floor_y
	s.y = tunnel_dims().y
	if depth >= Settings.BLOCK:
		var g := Settings.BLOCK
		var blk := floori(float(hit[ax]) / g) * g
		lo[ax] = blk + g - depth if normal[ax] > 0 else blk
	return AABB(Vector3(lo), Vector3(s))


static func _place_box(hit: Vector3i, front: Vector3i, size: int, shape: String) -> AABB:
	var normal := front - hit
	match shape:
		"flat":
			return _face_box(hit, normal, size, 1, true)
		"column":
			var d := column_dims(size)
			return _face_box(hit, normal, d.x, d.y, true)
	return _face_box(hit, normal, size, size, true)


static func _place_centre(hit: Vector3i, front: Vector3i, size: int) -> Vector3:
	var n := Vector3(front - hit)
	return Vector3(front) + Vector3(0.5, 0.5, 0.5) + n * (size * 0.5 - 0.5)


static func _box(o: Variant, sz: Variant) -> Array[Vector3i]:
	var org := Vector3i(o)
	var s := Vector3i(sz)
	var out: Array[Vector3i] = []
	for z in s.z:
		for y in s.y:
			for x in s.x:
				out.append(org + Vector3i(x, y, z))
	return out


static func _round(c: Vector3, size: int, shape: String) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var r := size * 0.5
	var r2 := r * r + 0.01
	var lo := Vector3i((c - Vector3.ONE * r).floor())
	var hi := Vector3i((c + Vector3.ONE * r).ceil())
	for z in range(lo.z, hi.z):
		for y in range(lo.y, hi.y):
			for x in range(lo.x, hi.x):
				var d := Vector3(x + 0.5, y + 0.5, z + 0.5) - c
				var inside := d.length_squared() <= r2 if shape == "sphere" else (d.x * d.x + d.z * d.z <= r2 and absf(d.y) <= r)
				if inside:
					out.append(Vector3i(x, y, z))
	return out
