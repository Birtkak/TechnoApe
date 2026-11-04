class_name FallStreaks
extends Node3D

## Falling liquid as one stream (ADR 0138). A falling cell jumps up to seven cells a sim tick and a stream thins as
## it speeds up, so the cubes the mesher draws for it (ADR 0103) have gaps that step 30 times a second. After every
## tick SimWorld.get_fall_streaks lists the air each falling cell crossed, merged per column; one MultiMesh per
## liquid draws those runs as open boxes (side faces only) in the liquid's own material, whose side faces already run
## lanes of foam downwards, so the gaps fill and the stream reads as continuous water. Presentation only.

const WIDTH := 0.94   # a hair inside the cubes: no coplanar faces to flicker against a falling neighbour

var _mm := {}   # fluid slot -> MultiMesh
var _materials := {}


## `materials`: World.fluid_materials (fluid slot -> material).
func setup(materials: Dictionary) -> void:
	_materials = materials
	for slot in materials:
		var mmi := MultiMeshInstance3D.new()
		mmi.name = "Streaks%d" % slot
		mmi.multimesh = _new_multimesh()
		mmi.material_override = materials[slot]
		mmi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		mmi.extra_cull_margin = 16384.0   # the AABB of a MultiMesh does not follow a buffer set from code
		add_child(mmi)
		_mm[slot] = mmi.multimesh


## After a finished sim tick: the window's streaks; `offset` turns window cells into world voxels.
func update(sim: SimWorld, offset: Vector3) -> void:
	for slot in _mm:
		var buf: PackedFloat32Array = sim.get_fall_streaks(slot, offset, WIDTH)
		var mm: MultiMesh = _mm[slot]
		var n := buf.size() / 12
		if n == 0 and mm.visible_instance_count == 0:
			continue
		if n > mm.instance_count:
			mm.instance_count = maxi(64, nearest_po2(n))
		buf.resize(mm.instance_count * 12)   # the rest are zero transforms (drawn as nothing)
		mm.buffer = buf
		mm.visible_instance_count = n


## For MaterialWarmup: one tiny streak per liquid, so the instanced pipeline compiles under the loading screen.
func warm_nodes() -> Array:
	var out := []
	for slot in _mm:
		var mmi := MultiMeshInstance3D.new()
		mmi.multimesh = _new_multimesh()
		mmi.multimesh.instance_count = 1
		mmi.multimesh.set_instance_transform(0, Transform3D(Basis.from_scale(Vector3.ONE * 0.002), Vector3.ZERO))
		mmi.material_override = _materials[slot]
		out.append(mmi)
	return out


static func _new_multimesh() -> MultiMesh:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.mesh = _tube()
	return mm


## A unit box without top and bottom: x and z in -0.5..0.5, y in 0..1, faces outward.
static func _tube() -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var sides := [[Vector3(1, 0, 0), Vector3(0, 0, -1)], [Vector3(-1, 0, 0), Vector3(0, 0, 1)],
			[Vector3(0, 0, 1), Vector3(1, 0, 0)], [Vector3(0, 0, -1), Vector3(-1, 0, 0)]]
	for s in sides:
		var n: Vector3 = s[0]
		var t: Vector3 = s[1]   # along the face: q runs counter-clockwise seen from outside, Godot's front is clockwise
		var c := n * 0.5
		var q := [c - t * 0.5, c + t * 0.5, c + t * 0.5 + Vector3.UP, c - t * 0.5 + Vector3.UP]
		st.set_normal(n)
		st.set_uv(Vector2(0, 8))   # UV.y 8 = no flow state (terrain_fluid.gdshaderinc)
		for k in [0, 2, 1, 0, 3, 2]:
			st.add_vertex(q[k])
	return st.commit()
