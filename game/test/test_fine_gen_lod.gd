extends GdUnitTestSuite

## FineGen at every LOD (ADR 0017's rules on the 12.5 cm grid, ADR 0033): LOD 0 has a top, about a
## metre of soil, then rock; a coarse cell holding the surface carries the surface material and
## nothing under it is grass or (beyond LOD 1's cave mouths) air; far forests are leaf domes
## standing on the ground (ADR 0035); LOD 1 carves exactly LOD 0's caves near the surface.

const SEED := 4242
const SY := 32   # cells per test column
const WOODY := [GameMaterials.WOOD, GameMaterials.LEAVES, GameMaterials.OAK_BARK, GameMaterials.BIRCH_LOG, GameMaterials.BIRCH_LEAVES,
	GameMaterials.BIRCH_BARK, GameMaterials.SPRUCE_LOG, GameMaterials.SPRUCE_LEAVES, GameMaterials.SPRUCE_BARK, GameMaterials.MOSS,
	GameMaterials.BLUEBERRY_BUSH]


func _gen() -> FineGen:
	var g := FineGen.new()
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
func _column_at(g: FineGen, wx: int, wz: int, y0: int, lod: int) -> PackedInt32Array:
	var size := Vector3i(1, SY, 1)
	var bytes: PackedByteArray = g.generate_block(Vector3i(wx, y0, wz), size, lod, 2)
	if bytes.is_empty():
		var air := PackedInt32Array()
		air.resize(SY)
		return air
	return _col(bytes, size, 0, 0)


## Open grassland columns: grass on top, nothing above it for 3 m, with their surface height.
func _grass_columns(g: FineGen, want: int) -> Array:
	var found := []
	var x := 0
	while found.size() < want and x < 80000:
		var h: int = g.height_at(x, 7)
		if h > Settings.SEA_LEVEL_VOXEL + 16:
			var c := _column_at(g, x, 7, h - 8, 0)
			var clear := true
			for k in range(8, SY):
				clear = clear and c[k] == GameMaterials.AIR
			if c[7] == GameMaterials.GRASS and clear:
				found.append([x, 7, h])
		x += 149
	return found


func test_lod0_layers_are_top_soil_rock() -> void:
	var g := _gen()
	var cols := _grass_columns(g, 8)
	assert_int(cols.size()).is_greater_equal(4)
	for cz in cols:
		var c := _column_at(g, cz[0], cz[1], int(cz[2]) - SY + 1, 0)   # c[SY - 2] is the top voxel
		var top := SY - 2
		assert_int(c[top]).is_equal(GameMaterials.GRASS)
		for d in range(1, 8):                                       # about a metre of soil
			assert_int(c[top - d]).override_failure_message("soil at depth %d" % d).is_equal(GameMaterials.DIRT)
		for d in range(13, 20):                                     # then rock, ore or a cave
			assert_int(c[top - d]).is_not_equal(GameMaterials.DIRT)
			assert_int(c[top - d]).is_not_equal(GameMaterials.GRASS)


func test_coarse_surface_cell_keeps_the_surface_material() -> void:
	var g := _gen()
	var cols := _grass_columns(g, 16)
	assert_int(cols.size()).is_greater_equal(8)
	for lod: int in [1, 2, 3]:
		var step := 1 << lod
		for cz in cols:
			var y0: int = ((int(cz[2]) - 20 * step) / step) * step
			var c := _column_at(g, cz[0], cz[1], y0, lod)
			# a tree, root or log in this lod's cell of the column (coarse cells grow): not a ground test
			var woody := false
			for y in SY:
				woody = woody or c[y] in WOODY
			if woody:
				continue
			# the highest solid cell (coarse lods interpolate their own surface)
			var k := -1
			for y in SY:
				if c[y] != GameMaterials.AIR and c[y] != GameMaterials.TALL_GRASS:
					k = y
			if k < 0:
				continue
			if c[k] == GameMaterials.STONE and GameMaterials.GRASS in c.slice(0, k):
				continue   # a boulder on the grass
			assert_int(c[k]).override_failure_message("lod %d at (%d,%d): surface cell is %s" % [lod, cz[0], cz[1], GameMaterials.name_of(c[k])]).is_equal(GameMaterials.GRASS)
			for j in range(0, k):
				if c[j] == GameMaterials.GRASS:   # only as the floor of a shallow cave mouth
					assert_int(c[j + 1]).is_equal(GameMaterials.AIR)
				if lod > 1 and c[j] == GameMaterials.AIR:
					assert_int(j).override_failure_message("lod %d: air under the ground" % lod).is_less(0)


func test_far_lod_forests_are_leaf_domes_on_the_ground() -> void:
	# From lod 4 trees have no geometry: a tree's cell carries a dome of leaves standing on the
	# ground (ADR 0035). Scan lod-4 blocks over forest: leaves exist, none float.
	var g := _gen()
	var p: Vector3i = g.find_nearest("forest", 0, 0, int(Settings.m2v(8000.0)))
	assert_int(p.y).is_greater(0)
	var size := Vector3i(16, 16, 16)
	var leaves := 0
	var floating := 0
	for t in 6:
		var x := p.x + (t - 3) * 256
		var h: int = g.height_at(x + 128, p.z + 128)
		var y0 := ((h - 64) / 16) * 16
		var bytes: PackedByteArray = g.generate_block(Vector3i(x, y0, p.z), size, 4, 2)
		if bytes.is_empty():
			continue
		for bz in size.z:
			for bx in size.x:
				var c := _col(bytes, size, bx, bz)
				for y in range(1, size.y):
					if c[y] == GameMaterials.LEAVES:
						leaves += 1
						if c[y - 1] == GameMaterials.AIR:
							floating += 1
	assert_int(leaves).is_greater(0)
	assert_int(floating).is_equal(0)


func test_lod1_keeps_cave_mouths_near_the_surface() -> void:
	# lod 1 carves caves within 3 m of the surface: where the lod-0 voxel at an even position is
	# cave air under ground, the lod-1 cell there is air too; deeper than the band it is not.
	var g := _gen()
	var band := 24 * FineGen.FS
	var size0 := Vector3i(16, 16, 16)
	var checked := 0
	var agree := 0
	var deep_carved := 0
	var x := 0
	while x < 400000 and checked < 40:
		var h: int = g.height_at(x, 3)
		if h > Settings.SEA_LEVEL_VOXEL + 40:
			var y0 := ((h - 64) / 16) * 16
			var b0: PackedByteArray = g.generate_block(Vector3i(x, y0, 3), size0, 0, 2)
			var y1 := (y0 / 32) * 32
			var b1: PackedByteArray = g.generate_block(Vector3i(x, y1, 3), size0, 1, 2)
			if not b0.is_empty() and not b1.is_empty():
				var c0 := _col(b0, size0, 0, 0)
				var c1 := _col(b1, size0, 0, 0)
				for y in range(0, 16, 2):
					var wy := y0 + y
					if wy < h - 1 and h - wy <= band and c0[y] == GameMaterials.AIR:
						checked += 1
						if (wy - y1) % 2 == 0 and c1[(wy - y1) / 2] == GameMaterials.AIR:
							agree += 1
				for k in 16:
					var wyk := y1 + k * 2
					if wyk < h - band - 2 and wyk > 3 * FineGen.FS and c1[k] == GameMaterials.AIR:
						deep_carved += 1
		x += 245
	assert_int(checked).is_greater(0)
	assert_int(agree).is_equal(checked)
	assert_int(deep_carved).is_equal(0)
