extends GdUnitTestSuite

## ShapeTool (ADR 0038): the build shapes on the voxel grid, turned, mirrored, hollowed, written into
## the SimWorld only where it is free, and read back for prefabs.

var st := ShapeTool.new()


func _count(params: Dictionary) -> int:
	return (st.cells(params, Vector3i.ZERO)["cells"] as PackedInt32Array).size() / 3


## Cells per layer (y) of a shape at lo = 0.
func _layers(params: Dictionary) -> Dictionary:
	var out := {}
	var c: PackedInt32Array = st.cells(params, Vector3i.ZERO)["cells"]
	for i in range(0, c.size(), 3):
		out[c[i + 1]] = int(out.get(c[i + 1], 0)) + 1
	return out


func _inside_box(r: Dictionary, lo: Vector3i) -> bool:
	var c: PackedInt32Array = r["cells"]
	var s: Vector3i = r["size"]
	for i in range(0, c.size(), 3):
		var p := Vector3i(c[i], c[i + 1], c[i + 2]) - lo
		if p.x < 0 or p.y < 0 or p.z < 0 or p.x >= s.x or p.y >= s.y or p.z >= s.z:
			return false
	return true


func test_a_box_fills_its_size_and_hollow_leaves_the_walls() -> void:
	assert_int(_count({"shape": "box", "size": Vector3i(4, 3, 5)})).is_equal(60)
	# a ring wall 1 thick around a 6 x 6 floor plan, 2 high: (36 - 16) per layer
	assert_int(_count({"shape": "box", "size": Vector3i(6, 2, 6), "hollow": 1})).is_equal(40)


func test_turning_swaps_the_sides_and_keeps_the_cells() -> void:
	var y1 := st.turn(0, 1)
	assert_that(st.oriented_size(Vector3i(4, 3, 5), y1)).is_equal(Vector3i(5, 3, 4))
	assert_that(st.local_size(Vector3i(5, 3, 4), y1)).is_equal(Vector3i(4, 3, 5))
	var r := st.cells({"shape": "stairs", "size": Vector3i(4, 8, 8), "rot": y1}, Vector3i(10, 20, 30))
	assert_int((r["cells"] as PackedInt32Array).size() / 3).is_equal(_count({"shape": "stairs", "size": Vector3i(4, 8, 8)}))
	assert_bool(_inside_box(r, Vector3i(10, 20, 30))).is_true()
	# four quarter turns about up are no turn; every rotation is reachable and in 0..23
	var rot := 0
	for i in 4:
		rot = st.turn(rot, 1)
	assert_int(rot).is_equal(0)
	var seen := {}
	var front := [0]
	while not front.is_empty():
		var q: int = front.pop_back()
		if seen.has(q):
			continue
		seen[q] = true
		for axis in 3:
			front.append(st.turn(q, axis))
	assert_int(seen.size()).is_equal(24)


func test_tipping_a_wall_lays_it_down() -> void:
	var tipped := st.turn(0, 0)   # about x: height becomes depth
	assert_that(st.oriented_size(Vector3i(16, 12, 2), tipped)).is_equal(Vector3i(16, 2, 12))
	var r := st.cells({"shape": "wall", "size": Vector3i(16, 12, 2), "rot": tipped}, Vector3i.ZERO)
	assert_bool(_inside_box(r, Vector3i.ZERO)).is_true()


func test_mirror_flips_stairs_and_keeps_their_box() -> void:
	var p := {"shape": "stairs", "size": Vector3i(8, 8, 8), "rot": st.turn(0, 1)}   # rising along x
	var a := st.cells(p, Vector3i.ZERO)
	p["mirror"] = true
	var b := st.cells(p, Vector3i.ZERO)
	assert_bool(_inside_box(b, Vector3i.ZERO)).is_true()
	# the tall end moved to the other side: the column at x = 0 differs
	var col := func(r: Dictionary) -> int:
		var c: PackedInt32Array = r["cells"]
		var n := 0
		for i in range(0, c.size(), 3):
			n += 1 if c[i] == 0 else 0
		return n
	assert_int(col.call(a)).is_not_equal(col.call(b))


func test_round_shapes() -> void:
	var sphere := _count({"shape": "sphere", "size": Vector3i(8, 8, 8)})
	assert_int(sphere).is_between(230, 300)   # 4/3 pi 4^3 = 268
	assert_int(_count({"shape": "sphere", "size": Vector3i(8, 8, 8), "hollow": 1})).is_less(sphere)
	var cyl := _layers({"shape": "cylinder", "size": Vector3i(8, 4, 8)})
	assert_int(cyl.size()).is_equal(4)
	assert_int(int(cyl[0])).is_equal(int(cyl[3]))   # straight sides
	assert_int(int(cyl[0])).is_between(44, 56)      # pi 4^2 = 50
	var cone := _layers({"shape": "cone", "size": Vector3i(8, 8, 8)})
	assert_int(int(cone[0])).is_greater(int(cone[6]))


func test_stairs_and_slope_rise_along_depth() -> void:
	# two 0.5 m steps over 1 m: the near half 4 high, the far half 8 high, 4 wide
	assert_int(_count({"shape": "stairs", "size": Vector3i(4, 8, 8), "step": 4})).is_equal(4 * (4 * 4 + 4 * 8))
	var c: PackedInt32Array = st.cells({"shape": "slope", "size": Vector3i(1, 8, 8)}, Vector3i.ZERO)["cells"]
	var heights := {}
	for i in range(0, c.size(), 3):
		heights[c[i + 2]] = maxi(int(heights.get(c[i + 2], -1)), c[i + 1])
	for z in range(1, 8):
		assert_int(int(heights.get(z, -1))).is_greater_equal(int(heights.get(z - 1, -1)))


func test_fill_writes_only_free_cells_and_reads_back_as_a_prefab() -> void:
	var sim := SimWorld.new()
	sim.configure(Vector3i(2, 2, 2), 1, 1)
	sim.set_materials(GameMaterials.defs())
	sim.mark_all_loaded(true)
	var mask := PackedByteArray()
	mask.resize(256)
	mask[GameMaterials.AIR] = 1
	sim.set_voxel(Vector3i(2, 0, 2), GameMaterials.STONE)   # in the way
	var box := st.cells({"shape": "box", "size": Vector3i(4, 2, 4)}, Vector3i.ZERO)
	assert_int(st.count_free(sim, box["cells"], Vector3i.ZERO, mask)).is_equal(31)
	assert_int(st.fill(sim, box["cells"], GameMaterials.WOOD, PackedByteArray(), Vector3i.ZERO, mask)).is_equal(31)
	assert_int(sim.get_voxel(Vector3i(2, 0, 2))).is_equal(GameMaterials.STONE)
	# capture it and place it elsewhere, turned: the same materials come out
	var grid := st.read(sim, Vector3i.ZERO, Vector3i(4, 2, 4), Vector3i.ZERO)
	var copy := st.cells({"shape": "grid", "size": Vector3i(4, 2, 4), "grid": grid, "rot": st.turn(0, 1)}, Vector3i(10, 0, 10))
	assert_int((copy["mats"] as PackedByteArray).size()).is_equal(32)
	var by: Dictionary = st.count_free_by_material(sim, copy["cells"], copy["mats"], Vector3i.ZERO, mask)
	assert_int(int(by[GameMaterials.WOOD])).is_equal(31)
	assert_int(int(by[GameMaterials.STONE])).is_equal(1)
	st.fill(sim, copy["cells"], 0, copy["mats"], Vector3i.ZERO, mask)
	assert_int(sim.count_in_box(Vector3i(10, 0, 10), Vector3i(14, 2, 14), GameMaterials.WOOD)).is_equal(31)


func test_the_preview_mesh_has_only_outer_faces() -> void:
	var one := st.cells({"shape": "box", "size": Vector3i(1, 1, 1)}, Vector3i(5, 5, 5))
	var m: Array = st.mesh(one["cells"], Vector3i(5, 5, 5), one["size"])
	assert_int((m[Mesh.ARRAY_VERTEX] as PackedVector3Array).size()).is_equal(24)
	var two := st.cells({"shape": "box", "size": Vector3i(2, 1, 1)}, Vector3i.ZERO)
	m = st.mesh(two["cells"], Vector3i.ZERO, two["size"])
	assert_int((m[Mesh.ARRAY_INDEX] as PackedInt32Array).size()).is_equal(10 * 6)
