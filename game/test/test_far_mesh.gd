extends GdUnitTestSuite

## The far layer's data path (ADR 0074, native FarMesh): a tile is a full grid of columns with unit
## normals and material colours; the parent heights and colours it carries are exactly what the parent
## tile draws at those positions (so the CDLOD morph meets the next ring without cracks); water sheets
## lie on their bed, trees stand on the ground and come with MultiMesh buffers; the surface-nets
## candidate yields a closed surface in the same height range.

const SEED := 272905368   # the ADR 0072 / 0073 report world: a river, a beach and forest around (-160, -277)
const X := -160
const Z := -277


func _mesh() -> FarMesh:
	var g := FineGen.new()
	g.configure(SEED, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var fm := FarMesh.new()
	fm.configure(g)
	fm.set_material_colors(FarColors.material_colors())
	return fm


func _tile_at(fm: FarMesh, level: int, x: int, z: int) -> Dictionary:
	var size := fm.tile_voxels(level)
	return fm.build_tile(level, int(floor(float(x) / size)), int(floor(float(z) / size)))


func test_tile_is_a_full_grid_of_coloured_columns() -> void:
	var fm := _mesh()
	var d := _tile_at(fm, 0, X, Z)
	var n := FarMesh.CELLS + 1
	var verts: PackedVector3Array = d["vertices"]
	assert_int(verts.size()).is_equal(n * n)
	assert_int((d["indices"] as PackedInt32Array).size()).is_equal(FarMesh.CELLS * FarMesh.CELLS * 6)
	var norms: PackedVector3Array = d["normals"]
	var cols: PackedColorArray = d["colors"]
	var plain := 0
	for i in verts.size():
		assert_float(verts[i].y).is_between(0.0, float(Settings.WORLD_HEIGHT_VOXELS))
		assert_float(norms[i].length()).is_equal_approx(1.0, 0.01)
		assert_float(norms[i].y).is_greater(0.0)
		var c := cols[i]
		if c.r + c.g + c.b > 2.7 or c.r + c.g + c.b < 0.01:
			plain += 1
	assert_int(plain).override_failure_message("vertices with white or black colours: %d" % plain).is_equal(0)
	assert_int((d["custom0"] as PackedByteArray).size()).is_equal(n * n * 4)
	assert_int((d["uvs2"] as PackedVector2Array).size()).is_equal(n * n)


func test_parent_heights_and_colours_are_the_parent_tiles_own() -> void:
	var fm := _mesh()
	for level in [0, 1, 3]:
		var size := fm.tile_voxels(level)
		var tx := int(floor(float(X) / size))
		var tz := int(floor(float(Z) / size))
		var child: Dictionary = fm.build_tile(level, tx, tz)
		var parent: Dictionary = fm.build_tile(level + 1, tx >> 1, tz >> 1)
		var pv: PackedVector3Array = parent["vertices"]
		var pc: PackedColorArray = parent["colors"]
		var by_pos := {}
		for i in pv.size():
			by_pos[Vector2i(int(pv[i].x), int(pv[i].z))] = i
		var cv: PackedVector3Array = child["vertices"]
		var uv: PackedVector2Array = child["uvs"]
		var cu: PackedByteArray = child["custom0"]
		var checked := 0
		var n := FarMesh.CELLS + 1
		for cj in range(0, n, 2):
			for ci in range(0, n, 2):
				var k := cj * n + ci
				var key := Vector2i(int(cv[k].x), int(cv[k].z))
				assert_bool(by_pos.has(key)).override_failure_message("level %d child vertex %s has no parent vertex" % [level, key]).is_true()
				var pi: int = by_pos[key]
				assert_float(uv[k].x).override_failure_message("level %d parent height at %s" % [level, key]).is_equal_approx(pv[pi].y, 0.01)
				var col := pc[pi]
				assert_int(cu[k * 4]).is_between(int(round(col.r * 255.0)) - 1, int(round(col.r * 255.0)) + 1)
				assert_int(cu[k * 4 + 1]).is_between(int(round(col.g * 255.0)) - 1, int(round(col.g * 255.0)) + 1)
				checked += 1
		assert_int(checked).is_equal(33 * 33)


func test_water_sheets_lie_on_their_bed_and_at_one_level_per_cell() -> void:
	var fm := _mesh()
	var found := false
	for level in [1, 2]:
		var d := _tile_at(fm, level, X, Z)
		if not d.has("water_vertices"):
			continue
		found = true
		var wv: PackedVector3Array = d["water_vertices"]
		var wc: PackedColorArray = d["water_colors"]
		var gv: PackedVector3Array = d["vertices"]
		var n := FarMesh.CELLS + 1
		var size := fm.tile_voxels(level)
		var step := size / FarMesh.CELLS
		var x0 := int(floor(float(X) / size)) * size
		var z0 := int(floor(float(Z) / size)) * size
		assert_int(wv.size() % 4).is_equal(0)
		for q in range(0, wv.size(), 4):
			var y := wv[q].y
			var wet := 0
			for k in 4:
				assert_float(wv[q + k].y).is_equal_approx(y, 0.001)   # flat per cell
				var ci := int(round((wv[q + k].x - x0) / step))
				var cj := int(round((wv[q + k].z - z0) / step))
				var ground := gv[cj * n + ci].y
				if ground < y - 0.2:
					wet += 1
				assert_float(wc[q + k].a).is_between(0.0, 1.0)
			# a sheet exists only where a corner is under water; dry corners of the same cell are its bank,
			# which the ground hides (the sheet dives under it)
			assert_int(wet).override_failure_message("a water sheet with no wet corner at %s" % wv[q]).is_greater(0)
	assert_bool(found).override_failure_message("no water tile near the river at (%d, %d)" % [X, Z]).is_true()


func test_trees_stand_on_the_ground_with_multimesh_buffers() -> void:
	var fm := _mesh()
	var g := FineGen.new()
	g.configure(SEED, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var d := _tile_at(fm, 2, X, Z)
	var tf: PackedFloat32Array = d["trees"]
	var n := tf.size() / 7
	assert_int(n).override_failure_message("no trees in the forest tile").is_greater(0)
	assert_int((d["crowns"] as PackedFloat32Array).size()).is_equal(n * 16)
	assert_int((d["trunks"] as PackedFloat32Array).size()).is_equal(0)   # trunks only to level 1
	for i in n:
		var o := i * 7
		var ground := g.height_at(int(floor(tf[o])), int(floor(tf[o + 2])))
		assert_float(tf[o + 1]).override_failure_message("tree %d floats %.1f voxels from the ground" % [i, tf[o + 1] - ground]).is_between(ground - 6.0, ground + 6.0)
		assert_float(tf[o + 3]).is_greater(2.0)      # crown radius
		assert_float(tf[o + 4]).is_greater(8.0)      # crown height
		assert_float(tf[o + 6]).is_between(0.0, 1.0) # age
	var d1 := _tile_at(fm, 1, X, Z)
	var n1 := (d1["trees"] as PackedFloat32Array).size() / 7
	assert_int((d1["trunks"] as PackedFloat32Array).size()).is_equal(n1 * 16)


func test_coarse_levels_bake_the_canopy_and_stay_in_range() -> void:
	var fm := _mesh()
	var d3 := _tile_at(fm, FarMesh.TREE_LEVEL + 1, X, Z)
	var d2 := _tile_at(fm, FarMesh.TREE_LEVEL, X, Z)
	assert_bool(d3.has("trees")).is_false()
	var top3 := -1e9
	for v in (d3["vertices"] as PackedVector3Array):
		assert_float(v.y).is_between(0.0, float(Settings.WORLD_HEIGHT_VOXELS))
		top3 = maxf(top3, v.y)
	var top2 := -1e9
	for v in (d2["vertices"] as PackedVector3Array):
		top2 = maxf(top2, v.y)
	# the forest's canopy raises the skin where the last instance level still draws trees
	assert_float(top3).override_failure_message("baked top %.0f vs instance level top %.0f" % [top3, top2]).is_greater(top2 + Settings.m2v(4.0))


func test_surface_nets_give_a_closed_surface_in_the_same_range() -> void:
	var fm := _mesh()
	var size := fm.tile_voxels(2)
	var tx := int(floor(float(X) / size))
	var tz := int(floor(float(Z) / size))
	var d: Dictionary = fm.build_nets_tile(2, tx, tz)
	var verts: PackedVector3Array = d["vertices"]
	var idx: PackedInt32Array = d["indices"]
	assert_int(verts.size()).is_greater(1000)
	assert_int(idx.size() % 3).is_equal(0)
	var field: Dictionary = fm.build_tile(2, tx, tz)
	var lo := 1e9
	var hi := -1e9
	for v in (field["vertices"] as PackedVector3Array):
		lo = minf(lo, v.y)
		hi = maxf(hi, v.y)
	for i in idx:
		assert_int(i).is_between(0, verts.size() - 1)
	for v in verts:
		assert_float(v.y).is_between(lo - 40.0, hi + Settings.m2v(30.0))   # the voxel data has caves below and trees above the skin
