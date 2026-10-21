class_name ChunkGd
extends RefCounted

## Reference blocky-voxel chunk generator + naive culled mesher in typed GDScript.
## Same algorithm as ChunkCs.cs so the two can be compared fairly.

const SIZE := 16
const SIZE2 := SIZE * SIZE
const VOLUME := SIZE * SIZE * SIZE

const AIR := 0
const STONE := 1
const DIRT := 2
const GRASS := 3

# Corner tables per face, counter-clockwise seen from outside. Indices below
# flip them to clockwise, which is Godot's front-face winding.
const FACE_NORMALS: Array[Vector3] = [
	Vector3(1, 0, 0), Vector3(-1, 0, 0), Vector3(0, 1, 0),
	Vector3(0, -1, 0), Vector3(0, 0, 1), Vector3(0, 0, -1),
]
const FACE_OFFSETS: Array[Vector3i] = [
	Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 1, 0),
	Vector3i(0, -1, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1),
]
const FACE_CORNERS: Array = [
	[Vector3(1, 0, 0), Vector3(1, 1, 0), Vector3(1, 1, 1), Vector3(1, 0, 1)],
	[Vector3(0, 0, 0), Vector3(0, 0, 1), Vector3(0, 1, 1), Vector3(0, 1, 0)],
	[Vector3(0, 1, 0), Vector3(0, 1, 1), Vector3(1, 1, 1), Vector3(1, 1, 0)],
	[Vector3(0, 0, 0), Vector3(1, 0, 0), Vector3(1, 0, 1), Vector3(0, 0, 1)],
	[Vector3(0, 0, 1), Vector3(1, 0, 1), Vector3(1, 1, 1), Vector3(0, 1, 1)],
	[Vector3(0, 0, 0), Vector3(0, 1, 0), Vector3(1, 1, 0), Vector3(1, 0, 0)],
]
const TYPE_COLORS: Array[Color] = [
	Color.BLACK, Color(0.5, 0.5, 0.52), Color(0.45, 0.3, 0.15), Color(0.25, 0.6, 0.2),
]


static func index(x: int, y: int, z: int) -> int:
	return x + SIZE * (y + SIZE * z)


## Fills a chunk at chunk coordinates (cx, cy, cz). Height from 2D noise,
## caves carved by 3D noise. Deterministic for a given noise seed.
static func generate(cx: int, cy: int, cz: int, noise2d: FastNoiseLite, noise3d: FastNoiseLite) -> PackedByteArray:
	var voxels := PackedByteArray()
	voxels.resize(VOLUME)
	var base_y := cy * SIZE
	for z in SIZE:
		var wz := cz * SIZE + z
		for x in SIZE:
			var wx := cx * SIZE + x
			var h := int(noise2d.get_noise_2d(float(wx), float(wz)) * 24.0 + 32.0)
			for y in SIZE:
				var wy := base_y + y
				var v := AIR
				if wy < h - 4:
					v = STONE
				elif wy < h - 1:
					v = DIRT
				elif wy < h:
					v = GRASS
				if v != AIR and noise3d.get_noise_3d(float(wx), float(wy), float(wz)) > 0.55:
					v = AIR
				voxels[x + SIZE * (y + SIZE * z)] = v
	return voxels


## Naive culled mesher: one quad per solid face touching air. Neighbours
## outside the chunk count as air (border faces are emitted; same in C#).
## Returns surface arrays ready for ArrayMesh.add_surface_from_arrays.
static func mesh(voxels: PackedByteArray) -> Array:
	var verts := PackedVector3Array()
	var normals := PackedVector3Array()
	var colors := PackedColorArray()
	var indices := PackedInt32Array()
	var corners: Array = FACE_CORNERS
	for z in SIZE:
		for y in SIZE:
			for x in SIZE:
				var v := voxels[x + SIZE * (y + SIZE * z)]
				if v == AIR:
					continue
				var col: Color = TYPE_COLORS[v]
				var pos := Vector3(x, y, z)
				for f in 6:
					var o: Vector3i = FACE_OFFSETS[f]
					var nx := x + o.x
					var ny := y + o.y
					var nz := z + o.z
					if nx >= 0 and nx < SIZE and ny >= 0 and ny < SIZE and nz >= 0 and nz < SIZE:
						if voxels[nx + SIZE * (ny + SIZE * nz)] != AIR:
							continue
					var base := verts.size()
					var n: Vector3 = FACE_NORMALS[f]
					var c: Array = corners[f]
					for k in 4:
						verts.push_back(pos + c[k])
						normals.push_back(n)
						colors.push_back(col)
					indices.push_back(base)
					indices.push_back(base + 2)
					indices.push_back(base + 1)
					indices.push_back(base)
					indices.push_back(base + 3)
					indices.push_back(base + 2)
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = verts
	arrays[Mesh.ARRAY_NORMAL] = normals
	arrays[Mesh.ARRAY_COLOR] = colors
	arrays[Mesh.ARRAY_INDEX] = indices
	return arrays


static func build_array_mesh(arrays: Array) -> ArrayMesh:
	var m := ArrayMesh.new()
	if (arrays[Mesh.ARRAY_VERTEX] as PackedVector3Array).size() > 0:
		m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	return m
