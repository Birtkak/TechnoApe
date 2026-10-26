class_name Brush
extends RefCounted

## The dig and build brush (ADR 0034, ADR 0065): which 12.5 cm voxels one dig or one
## placement touches. Pure logic, headless testable.
##   cube      a size^3 cube snapped to the size grid (size 4 = a 0.5 m block,
##             so building on the old block grid stays easy)
##   sphere    a ball of diameter `size` centred on the aimed voxel
##   cylinder  an upright cylinder, diameter and height `size`
##   flat      a size x size square in the aimed face's plane, one voxel deep (shaving a wall,
##             levelling a floor, laying a floor or a wall skin); snapped like the cube
##   column    a size/2 wide square bar 2 x size long along the aimed face's normal: into the
##             material when digging (a shaft down, a tunnel in), out of the face when placing (a
##             pillar up, a beam out)
## Digging takes the shape around the aimed voxel; placing sets it against the
## aimed face (the cube snaps around the voxel in front of the face). `normal` is the
## aimed face's direction (front - hit), only flat and column use it.
## Sizes up to 8 are survival's (MiningTable.dig_cap caps digging by tool tier); creative adds
## 16 and 32 (4 m), which go through the native ShapeTool (`region`) instead of GDScript cells.

const SHAPES := ["cube", "sphere", "cylinder", "flat", "column"]
const SIZES := [1, 2, 4, 8]
const CREATIVE_SIZES := [1, 2, 4, 8, 16, 32]
const NATIVE_ABOVE := 8   # brushes bigger than this are filled natively (ShapeTool), never cell by cell


## Sizes Ctrl + wheel steps through in a mode.
static func sizes_for(creative: bool) -> Array:
	return CREATIVE_SIZES if creative else SIZES


## Cells a dig at the aimed voxel `hit` touches.
static func dig_cells(hit: Vector3i, size: int, shape: String, normal := Vector3i.UP) -> Array[Vector3i]:
	if size <= 1:
		return [hit]
	if shape in ["cube", "flat", "column"]:
		var b := _dig_box(hit, size, shape, normal)
		return _box(b.position, b.size)
	return _round(Vector3(hit) + Vector3(0.5, 0.5, 0.5), size, shape)


## Cells a placement against the face of `hit` touches; `front` is the voxel
## in front of that face (VoxelRaycastResult.previous_position).
static func place_cells(hit: Vector3i, front: Vector3i, size: int, shape: String) -> Array[Vector3i]:
	if size <= 1:
		return [front]
	if shape in ["cube", "flat", "column"]:
		var b := _place_box(hit, front, size, shape)
		return _box(b.position, b.size)
	return _round(_place_centre(hit, front, size), size, shape)


## The world box (position, size in voxels) a dig covers: the highlight outline.
static func dig_bounds(hit: Vector3i, size: int, shape: String, normal := Vector3i.UP) -> AABB:
	if size <= 1:
		return AABB(Vector3(hit), Vector3.ONE)
	if shape in ["cube", "flat", "column"]:
		var b := _dig_box(hit, size, shape, normal)
		return AABB(Vector3(b.position), Vector3(b.size))
	var c := Vector3(hit) + Vector3(0.5, 0.5, 0.5)
	return AABB(c - Vector3.ONE * size * 0.5, Vector3.ONE * size)


## The box and ShapeTool params of a brush for the native path (big creative brushes and the
## preview): {lo: Vector3i, size: Vector3i, params: Dictionary for ShapeTool.cells(params, lo)}.
## Round shapes take an odd box (size + 1) centred on the voxel, like the GDScript cells.
static func region(hit: Vector3i, front: Vector3i, size: int, shape: String, place: bool) -> Dictionary:
	if shape in ["cube", "flat", "column"] or size <= 1:
		var b: AABB = AABB(Vector3(front if place else hit), Vector3.ONE)
		if size > 1:
			var bi := _place_box(hit, front, size, shape) if place else _dig_box(hit, size, shape, front - hit)
			b = AABB(Vector3(bi.position), Vector3(bi.size))
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


## A short description for the HUD and toasts: "sphere 1 m", "flat 1 x 1 m", "column 0.5 x 2 m".
static func describe(size: int, shape: String) -> String:
	match shape:
		"flat":
			return "flat %s square" % _len(size)
		"column":
			var d := column_dims(size)
			return "column %s wide, %s long" % [_len(d.x), _len(d.y)]
	return "%s %s" % [shape, _len(size)]


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


static func _dig_box(hit: Vector3i, size: int, shape: String, normal: Vector3i) -> AABB:
	var ax := _axis(normal)
	var lo := _snap(hit, size)
	var s := Vector3i.ONE * size
	if shape == "flat":
		lo[ax] = hit[ax]
		s[ax] = 1
	elif shape == "column":
		var d := column_dims(size)
		lo = _snap(hit, d.x)
		s = Vector3i.ONE * d.x
		s[ax] = d.y
		# into the material: away from the face
		lo[ax] = hit[ax] - d.y + 1 if normal[ax] >= 0 else hit[ax]
	return AABB(Vector3(lo), Vector3(s))


static func _place_box(hit: Vector3i, front: Vector3i, size: int, shape: String) -> AABB:
	var normal := front - hit
	var ax := _axis(normal)
	var lo := _snap(front, size)
	var s := Vector3i.ONE * size
	if shape == "flat":
		lo[ax] = front[ax]
		s[ax] = 1
	elif shape == "column":
		var d := column_dims(size)
		lo = _snap(front, d.x)
		s = Vector3i.ONE * d.x
		s[ax] = d.y
		# out of the face: towards the player
		lo[ax] = front[ax] if normal[ax] >= 0 else front[ax] - d.y + 1
	return AABB(Vector3(lo), Vector3(s))


static func _place_centre(hit: Vector3i, front: Vector3i, size: int) -> Vector3:
	var n := Vector3(front - hit)
	return Vector3(front) + Vector3(0.5, 0.5, 0.5) + n * (size * 0.5 - 0.5)


static func _snap(p: Vector3i, size: int) -> Vector3i:
	return Vector3i(floori(float(p.x) / size) * size, floori(float(p.y) / size) * size, floori(float(p.z) / size) * size)


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
