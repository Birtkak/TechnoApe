class_name Brush
extends RefCounted

## The dig and build brush (ADR 0034): which 12.5 cm voxels one dig or one
## placement touches. Pure logic, headless testable.
##   cube      a size^3 cube snapped to the size grid (size 4 = a 0.5 m block,
##             so building on the old block grid stays easy)
##   sphere    a ball of diameter `size` centred on the aimed voxel
##   cylinder  an upright cylinder, diameter and height `size`
## Digging takes the shape around the aimed voxel; placing sets it against the
## aimed face (the cube snaps around the voxel in front of the face).

const SHAPES := ["cube", "sphere", "cylinder"]
const SIZES := [1, 2, 4, 8]


## Cells a dig at the aimed voxel `hit` touches.
static func dig_cells(hit: Vector3i, size: int, shape: String) -> Array[Vector3i]:
	if size <= 1:
		return [hit]
	if shape == "cube":
		return _box(_snap(hit, size), Vector3i.ONE * size)
	return _round(Vector3(hit) + Vector3(0.5, 0.5, 0.5), size, shape)


## Cells a placement against the face of `hit` touches; `front` is the voxel
## in front of that face (VoxelRaycastResult.previous_position).
static func place_cells(hit: Vector3i, front: Vector3i, size: int, shape: String) -> Array[Vector3i]:
	if size <= 1:
		return [front]
	if shape == "cube":
		return _box(_snap(front, size), Vector3i.ONE * size)
	var n := Vector3(front - hit)
	var c := Vector3(front) + Vector3(0.5, 0.5, 0.5) + n * (size * 0.5 - 0.5)
	return _round(c, size, shape)


## The world box (position, size in voxels) a dig covers: the highlight outline.
static func dig_bounds(hit: Vector3i, size: int, shape: String) -> AABB:
	if size <= 1:
		return AABB(Vector3(hit), Vector3.ONE)
	if shape == "cube":
		return AABB(Vector3(_snap(hit, size)), Vector3.ONE * size)
	var c := Vector3(hit) + Vector3(0.5, 0.5, 0.5)
	return AABB(c - Vector3.ONE * size * 0.5, Vector3.ONE * size)


static func _snap(p: Vector3i, size: int) -> Vector3i:
	return Vector3i(floori(float(p.x) / size) * size, floori(float(p.y) / size) * size, floori(float(p.z) / size) * size)


static func _box(o: Vector3i, s: Vector3i) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	for z in s.z:
		for y in s.y:
			for x in s.x:
				out.append(o + Vector3i(x, y, z))
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
