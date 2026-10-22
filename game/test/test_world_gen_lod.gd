extends GdUnitTestSuite

## Native WorldGen at LOD > 0 (ADR 0017): a coarse cell that holds the surface
## voxel carries the surface material (grass, snow, sand), buried cells are soil
## or rock by majority, so far hillsides stop showing dirt risers under a thin
## top. LOD 0 keeps its layers: top, 3 soil voxels, rock.

const SEED := 4242
const SY := 32   # block height in cells


func _gen() -> WorldGen:
	var g := WorldGen.new()
	g.configure(SEED, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	return g


## Column of one block: cell index y at (x, z), 16-bit materials.
func _col(bytes: PackedByteArray, size: Vector3i, x: int, z: int) -> PackedInt32Array:
	var out := PackedInt32Array()
	out.resize(size.y)
	for y in size.y:
		var i := 2 * (y + size.y * (x + size.x * z))
		out[y] = bytes[i] | (bytes[i + 1] << 8)
	return out


## Materials of the column at (wx, wz) from world y0 upward, sampled at this lod.
func _column_at(g: WorldGen, wx: int, wz: int, y0: int, lod: int) -> PackedInt32Array:
	var size := Vector3i(1, SY, 1)
	var bytes: PackedByteArray = g.generate_block(Vector3i(wx, y0, wz), size, lod, 2)
	if bytes.is_empty():
		var air := PackedInt32Array()
		air.resize(SY)
		return air
	return _col(bytes, size, 0, 0)


## Grassland columns clear of trees and water, with their surface height.
func _grass_columns(g: WorldGen, want: int) -> Array:
	var found := []
	var x := 0
	while found.size() < want and x < 20000:
		var h: int = g.height_at(x, 7)
		if h > Settings.SEA_LEVEL_VOXEL + 4:
			var c := _column_at(g, x, 7, h - 8, 0)
			# surface at index 7 (world h - 1), nothing above it (no tree, no plant)
			if c[7] == GameMaterials.GRASS and c[8] == GameMaterials.AIR and c[12] == GameMaterials.AIR:
				found.append([x, 7, h])
		x += 37
	return found


func test_lod0_layers_are_top_soil_rock() -> void:
	var g := _gen()
	var cols := _grass_columns(g, 8)
	assert_int(cols.size()).is_greater_equal(4)
	for cz in cols:
		var c := _column_at(g, cz[0], cz[1], cz[2] - 8, 0)
		assert_int(c[7]).is_equal(GameMaterials.GRASS)   # depth 1
		for d in range(2, 5):                              # depth 2..4: soil
			assert_int(c[8 - d]).is_equal(GameMaterials.DIRT)
		assert_int(c[8 - 5]).is_not_equal(GameMaterials.DIRT)  # depth 5: rock or ore
		assert_int(c[8 - 5]).is_not_equal(GameMaterials.GRASS)


func test_coarse_surface_cell_keeps_the_surface_material() -> void:
	var g := _gen()
	var cols := _grass_columns(g, 24)
	assert_int(cols.size()).is_greater_equal(8)
	for lod: int in [1, 2, 3]:
		var step := 1 << lod
		for cz in cols:
			var h: int = cz[2]
			# block origin aligned to the lod grid, well below the surface
			var y0 := ((h - 8 * step) / step) * step
			var c := _column_at(g, cz[0], cz[1], y0, lod)
			# the cell holding world y = h - 1
			var k := (h - 1 - y0) / step
			assert_int(c[k]).override_failure_message("lod %d at (%d,%d) h=%d: surface cell is %d" % [lod, cz[0], cz[1], h, c[k]]).is_equal(GameMaterials.GRASS)
			assert_int(c[k + 1]).is_equal(GameMaterials.AIR)
			# below the surface cell: never grass, never air
			for j in range(0, k):
				assert_int(c[j]).is_not_equal(GameMaterials.GRASS)
				assert_int(c[j]).is_not_equal(GameMaterials.AIR)


func test_buried_cells_are_soil_or_rock_by_majority() -> void:
	var g := _gen()
	var cols := _grass_columns(g, 12)
	assert_int(cols.size()).is_greater_equal(4)
	for cz in cols:
		var h: int = cz[2]
		# lod 1 (2 voxels per cell): the cell under the surface cell holds
		# depths 2-3 or 3-4, all soil
		var y0 := ((h - 16) / 2) * 2
		var c := _column_at(g, cz[0], cz[1], y0, 1)
		var k := (h - 1 - y0) / 2
		assert_int(c[k - 1]).is_equal(GameMaterials.DIRT)
		# lod 3 (8 voxels per cell): the cell under the surface cell is mostly rock
		y0 = ((h - 64) / 8) * 8
		c = _column_at(g, cz[0], cz[1], y0, 3)
		k = (h - 1 - y0) / 8
		assert_int(c[k - 1]).is_not_equal(GameMaterials.DIRT)
		assert_int(c[k - 1]).is_not_equal(GameMaterials.GRASS)


func test_far_lod_forests_collapse_into_leaf_coloured_ground() -> void:
	# From lod 3 on trees have no geometry: a column under a canopy gets a LEAVES
	# surface cell instead (ADR 0020). Scan a strip of lod-3 blocks: leaves exist,
	# and every leaf cell is a surface cell (air above, ground below).
	var g := _gen()
	var size := Vector3i(16, 16, 16)
	var leaves := 0
	var surface_leaves := 0
	var x := 0
	while x < 40000 and leaves < 20:
		var h: int = g.height_at(x, 13)
		if h > Settings.SEA_LEVEL_VOXEL + 2:
			var y0 := ((h - 64) / 8) * 8
			var bytes: PackedByteArray = g.generate_block(Vector3i(x, y0, 13), size, 3, 2)
			if not bytes.is_empty():
				for bz in size.z:
					for bx in size.x:
						var c := _col(bytes, size, bx, bz)
						for y in range(1, size.y - 1):
							if c[y] == GameMaterials.LEAVES:
								leaves += 1
								if c[y + 1] == GameMaterials.AIR and c[y - 1] != GameMaterials.AIR and c[y - 1] != GameMaterials.LEAVES:
									surface_leaves += 1
		x += 128
	assert_int(leaves).is_greater(0)
	assert_int(surface_leaves).is_equal(leaves)
