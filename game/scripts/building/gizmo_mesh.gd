class_name GizmoMesh
extends RefCounted

## The build handles' meshes (ADR 0144, after Lay of the Land): chunky white blocks. An arrow up with a
## fat rounded head, a four-way cross for moving the whole shape, a ring of pixels around the foot.
## The handle under the cursor turns HOVER. Unit size; ShapeBuilder scales them with the distance.

const HOVER := Color(0.45, 0.85, 1.0)
const RING_CELLS := 240   # a big ring takes wider pixels: a rebuild stays a few ms


## An arrow along +Y around the origin: a thin stem and a rounded head.
static func arrow() -> ArrayMesh:
	var list := [[Vector3(0, -0.55, 0), Vector3(0.3, 1.0, 0.3), Color.WHITE]]
	_round(list, Vector3(0, 0.0, 0), 0.8, 0.14)
	_round(list, Vector3(0, 0.16, 0), 1.3, 0.2)
	_round(list, Vector3(0, 0.36, 0), 1.2, 0.2)
	_round(list, Vector3(0, 0.56, 0), 0.8, 0.2)
	_round(list, Vector3(0, 0.74, 0), 0.4, 0.16)
	return _boxes(list)


## Four stubby arrows in the ground plane, run together into a blob.
static func cross() -> ArrayMesh:
	var list := []
	_round(list, Vector3.ZERO, 0.95, 0.42)
	for d: Vector3 in [Vector3.RIGHT, Vector3.LEFT, Vector3.FORWARD, Vector3.BACK]:
		var side := Vector3(absf(d.z), 0, absf(d.x))   # across the arm
		var along := Vector3(absf(d.x), 0, absf(d.z))
		list.append([d * 0.62, side * 0.4 + along * 0.4 + Vector3.UP * 0.34, Color.WHITE])
		list.append([d * 0.92, side * 1.0 + along * 0.26 + Vector3.UP * 0.42, Color.WHITE])
		list.append([d * 1.14, side * 0.6 + along * 0.2 + Vector3.UP * 0.42, Color.WHITE])
		list.append([d * 1.3, side * 0.24 + along * 0.14 + Vector3.UP * 0.42, Color.WHITE])
	return _boxes(list)


## A flat ring of square pixels of about `width` around the origin in the XZ plane.
static func ring(radius: float, width: float) -> ArrayMesh:
	var w := maxf(width, TAU * radius / RING_CELLS)
	var seen := {}
	var list := []
	for i in ceili(TAU * radius / w * 2.0):
		var a := TAU * i / ceili(TAU * radius / w * 2.0)
		var cell := Vector2i(roundi(cos(a) * radius / w), roundi(sin(a) * radius / w))
		if not seen.has(cell):
			seen[cell] = true
			list.append([Vector3(cell.x * w, 0, cell.y * w), Vector3(w, w * 0.5, w), Color.WHITE])
	return _boxes(list)


## A layer with its corners cut: two crossed boxes.
static func _round(list: Array, at: Vector3, w: float, h: float) -> void:
	list.append([at, Vector3(w, h, w * 0.62), Color.WHITE])
	list.append([at, Vector3(w * 0.62, h, w), Color.WHITE])


static func _flat_quad(v: PackedVector3Array, a: Vector3, b: Vector3, c: Vector3, d: Vector3) -> void:
	v.append_array([a, b, c, a, c, d])


## [centre, size, colour] boxes in one surface; each face shaded by its side, as the handles are unshaded.
static func _boxes(list: Array) -> ArrayMesh:
	var v := PackedVector3Array()
	var c := PackedColorArray()
	for e in list:
		var o: Vector3 = e[0] - e[1] * 0.5
		var s: Vector3 = e[1]
		var col: Color = e[2]
		var p := [o, o + Vector3(s.x, 0, 0), o + Vector3(s.x, 0, s.z), o + Vector3(0, 0, s.z),
			o + Vector3(0, s.y, 0), o + Vector3(s.x, s.y, 0), o + s, o + Vector3(0, s.y, s.z)]
		for f in [[4, 5, 6, 7, 1.0], [0, 1, 2, 3, 0.62], [0, 1, 5, 4, 0.8], [2, 3, 7, 6, 0.8], [1, 2, 6, 5, 0.9], [3, 0, 4, 7, 0.9]]:
			_flat_quad(v, p[f[0]], p[f[1]], p[f[2]], p[f[3]])
			var shade := Color(col.r * f[4], col.g * f[4], col.b * f[4])
			for i in 6:
				c.append(shade)
	var m := ArrayMesh.new()
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = v
	arrays[Mesh.ARRAY_COLOR] = c
	m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	return m
