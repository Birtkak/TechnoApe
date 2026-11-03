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


func test_lod0_deep_rock_shortcut_matches_the_full_pass() -> void:
	# ADR 0105: a buried LOD 0 block may come back as one voxel (the block is that stone); wherever it does,
	# the per-voxel pass must give that stone in every voxel (no cave, ore, geode or second rock kind lost)
	var fast := _gen()
	fast.set_uniform_blocks(true)
	var full := _gen()
	var size := Vector3i(16, 16, 16)
	var hits := 0
	var checked := 0
	for i in 12:
		var x := -3000 + i * 517
		var z := 1800 - i * 433
		var h := fast.height_at(x, z)
		for y in range(64, h - 300, 160):
			var o := Vector3i(x, y, z)
			var b := fast.generate_block(o, size, 0, 2)
			checked += 1
			if b.size() != 2:
				continue
			hits += 1
			var m := b.decode_u16(0)
			var f := full.generate_block(o, size, 0, 2)
			for k in range(0, f.size(), 2):
				if f.decode_u16(k) != m:
					fail("block %s: shortcut says %d, voxel %d is %d" % [o, m, k / 2, f.decode_u16(k)])
					return
	assert_int(hits).override_failure_message("no buried block took the shortcut (%d checked)" % checked).is_greater(0)


## Bug report 2026-11-02 18-55-21: grass over a sinkhole stood in mid air, because the block over the ground had
## no cave field and never saw the hole the block under it opened. Seed and spot of the report; every plant
## voxel must stand on something.
func test_no_plants_in_mid_air_over_an_open_cave_top() -> void:
	var g := FineGen.new()
	g.configure(1015715378, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var plants := [GameMaterials.TALL_GRASS, GameMaterials.FERN]
	var floating := 0
	var seen := 0
	for bx in range(2016, 2064, 16):
		for bz in range(2256, 2304, 16):
			var lo: PackedByteArray = g.generate_block(Vector3i(bx, 1040, bz), Vector3i(16, 16, 16), 0, 2)
			var hi: PackedByteArray = g.generate_block(Vector3i(bx, 1056, bz), Vector3i(16, 16, 16), 0, 2)
			for z in 16:
				for x in 16:
					var col := PackedInt32Array()
					for blk in [lo, hi]:
						if blk.size() == 16 * 16 * 16 * 2:
							col.append_array(_col(blk, Vector3i(16, 16, 16), x, z))
						else:
							var fill := PackedInt32Array()
							fill.resize(16)
							fill.fill(blk.decode_u16(0) if blk.size() >= 2 else GameMaterials.AIR)   # empty: sky; one voxel: uniform
							col.append_array(fill)
					for y in range(1, col.size()):
						if col[y] in plants:
							seen += 1
							if col[y - 1] == GameMaterials.AIR:
								floating += 1
	assert_int(seen).override_failure_message("no plants near the sinkhole: the spot moved").is_greater(0)
	assert_int(floating).is_equal(0)


## Bug report 2026-11-02 18-57-25: the flower meadow grows tree-sized flowers.
func test_flower_meadow_grows_giant_flowers() -> void:
	var g := _gen()
	var p: Vector3i = g.find_nearest("flower_meadow", 0, 0, int(Settings.m2v(8000.0)))
	assert_int(p.y).override_failure_message("no flower meadow").is_greater_equal(0)
	var parts := {}
	for bx in range(p.x - 64, p.x + 64, 16):
		for bz in range(p.z - 64, p.z + 64, 16):
			var h := g.height_at(bx + 8, bz + 8)
			for by in range(floori(h / 16.0) * 16, floori(h / 16.0) * 16 + 64, 16):
				var b: PackedByteArray = g.generate_block(Vector3i(bx, by, bz), Vector3i(16, 16, 16), 0, 2)
				for k in range(0, b.size(), 2):
					var m := b.decode_u16(k)
					if m in [GameMaterials.GIANT_STEM, GameMaterials.GIANT_PETAL, GameMaterials.GIANT_HEART]:
						parts[m] = true
	assert_int(parts.size()).override_failure_message("giant flower parts found: %s" % [parts.keys()]).is_equal(3)


## Cave biomes are dressed (FineGen::cave_dressing): the dripstone cave grows dripstone, the lush cave vines and the
## fungal cave toadstools, and no spike or vine voxel hangs alone in the air (each touches something above or below).
func test_cave_biomes_are_dressed_and_nothing_floats() -> void:
	var g := FineGen.new()
	g.configure(1015715378, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var want := {"dripstone_cave": GameMaterials.DRIPSTONE, "lush_cave": GameMaterials.CAVE_VINE, "fungal_cave": GameMaterials.FUNGUS_STEM}
	var hanging := [GameMaterials.DRIPSTONE, GameMaterials.CAVE_VINE, GameMaterials.GLOW_BERRY_VINE, GameMaterials.FUNGUS_STEM]
	for biome in want:
		var p: Vector3i = g.find_nearest(biome, 0, 0, int(Settings.m2v(4000.0)))
		assert_int(p.y).override_failure_message("no %s" % biome).is_greater_equal(0)
		var o := Vector3i(p.x - 48, p.y - 24, p.z - 48)
		var size := Vector3i(96, 48, 96)
		var b: PackedByteArray = g.generate_block(o, size, 0, 2)
		var found := 0
		var alone := 0
		if b.size() == size.x * size.y * size.z * 2:
			for z in size.z:
				for x in size.x:
					for y in range(1, size.y - 1):
						var m := b.decode_u16(2 * (y + size.y * (x + size.x * z)))
						if m == want[biome]:
							found += 1
						if m in hanging and b.decode_u16(2 * (y - 1 + size.y * (x + size.x * z))) == 0 and b.decode_u16(2 * (y + 1 + size.y * (x + size.x * z))) == 0:
							alone += 1
		assert_int(found).override_failure_message("%s near %s grows no %d" % [biome, p, want[biome]]).is_greater(0)
		assert_int(alone).override_failure_message("%s: %d dressing voxels hang alone" % [biome, alone]).is_equal(0)


## Ore outcrops: a shallow vein shows crags veined with its ore above the ground near its centre.
func test_shallow_vein_shows_ore_outcrops() -> void:
	var g := FineGen.new()
	g.configure(1015715378, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var p: Vector3i = g.find_nearest("coal_vein", -3000, -3000, 6000)
	var ores := 0
	for bx in range(p.x - 128, p.x + 128, 32):
		for bz in range(p.z - 128, p.z + 128, 32):
			var h := g.height_at(bx + 16, bz + 16)
			var b: PackedByteArray = g.generate_block(Vector3i(bx, h - 8, bz), Vector3i(32, 40, 32), 0, 2)
			if b.size() < 32 * 40 * 32 * 2:
				continue
			for k in range(0, b.size(), 2):
				if b.decode_u16(k) == GameMaterials.COAL_ORE:
					var i := k / 2
					if h - 8 + i % 40 >= g.height_at(bx + (i / 40) % 32, bz + i / 1280):
						ores += 1
	assert_int(ores).override_failure_message("no coal above the ground near the vein at %s" % p).is_greater(0)


## FineGen.cave_biome_at names the cave biome of a cave voxel and gives "" for sky and solid ground.
func test_cave_biome_at_names_cave_voxels_only() -> void:
	var g := FineGen.new()
	g.configure(1015715378, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var p: Vector3i = g.find_nearest("lush_cave", 0, 0, int(Settings.m2v(4000.0)))
	var names := {}
	for dx in range(-8, 9, 4):
		for y in range(p.y - 24, p.y + 24):
			var n := g.cave_biome_at(p.x + dx, y, p.z)
			if n != "":
				names[n] = true
	assert_bool(names.has("lush_cave")).override_failure_message("names near %s: %s" % [p, names.keys()]).is_true()
	var h := g.height_at(p.x, p.z)
	assert_str(g.cave_biome_at(p.x, h + 40, p.z)).is_empty()
	assert_str(g.cave_biome_at(p.x, 3 * 8 + 2, p.z)).is_empty()   # down in the bedrock


## The abandoned mine ruin: rails and loot pots down in its gallery, under the ruin's floor.
func test_abandoned_mine_has_rails_and_pots_underground() -> void:
	var g := FineGen.new()
	g.configure(1015715378, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var p: Vector3i = g.find_nearest("ruin_mine", 0, 0, 8000)
	assert_int(p.y).override_failure_message("no mine").is_greater_equal(0)
	var r: Dictionary = g.ruin_at(p.x, p.z)
	var hx: int = r["half_x"] + 8
	var hz: int = r["half_z"] + 8
	var b: PackedByteArray = g.generate_block(Vector3i(p.x - hx, p.y - 48, p.z - hz), Vector3i(2 * hx, 48, 2 * hz), 0, 2)
	var rails := 0
	var pots := 0
	for k in range(0, b.size(), 2):
		var m := b.decode_u16(k)
		if m == GameMaterials.RUSTED_RAIL:
			rails += 1
		elif GameMaterials.defs()[m - 1].has("shatter") if m > 0 else false:
			pots += 1
	assert_int(rails).is_greater(20)
	assert_int(pots).is_greater(0)


## Rock formations: the badlands hold arches and hoodoos of banded terracotta standing over 1.5 m above the ground.
func test_badlands_stand_rock_formations() -> void:
	var g := FineGen.new()
	g.configure(1015715378, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var p: Vector3i = g.find_nearest("badlands", 0, 0, int(Settings.m2v(9000.0)))
	assert_int(p.y).override_failure_message("no badlands").is_greater_equal(0)
	var rock := {GameMaterials.id_of("terracotta"): true, GameMaterials.id_of("terracotta_red"): true, GameMaterials.id_of("terracotta_cream"): true}
	var tall := 0
	for bx in range(p.x - 256, p.x + 256, 32):
		for bz in range(p.z - 256, p.z + 256, 32):
			var h := g.height_at(bx + 16, bz + 16)
			var b: PackedByteArray = g.generate_block(Vector3i(bx, h + 4, bz), Vector3i(32, 48, 32), 0, 2)
			if b.size() < 32 * 48 * 32 * 2:
				continue
			for k in range(0, b.size(), 2):
				if rock.has(b.decode_u16(k)):
					var i := k / 2
					if h + 4 + i % 48 >= g.height_at(bx + (i / 48) % 32, bz + i / 1536) + 12:
						tall += 1
	assert_int(tall).override_failure_message("no formations near %s" % p).is_greater(0)


## Plants per biome: dune grass on the beach, heather on the highlands, sedge through the tundra's snow.
func test_open_biomes_grow_their_own_plants() -> void:
	var g := FineGen.new()
	g.configure(1015715378, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	for e in [["beach", "dune_grass"], ["highlands", "heather"], ["tundra", "tundra_sedge"]]:
		var p: Vector3i = g.find_nearest(e[0], 0, 0, int(Settings.m2v(9000.0)))
		var want := GameMaterials.id_of(String(e[1]))
		var n := 0
		for bx in range(p.x - 64, p.x + 64, 32):
			for bz in range(p.z - 64, p.z + 64, 32):
				var h := g.height_at(bx + 16, bz + 16)
				var b: PackedByteArray = g.generate_block(Vector3i(bx, h - 8, bz), Vector3i(32, 24, 32), 0, 2)
				if b.size() < 32 * 24 * 32 * 2:
					continue
				for k in range(0, b.size(), 2):
					n += int(b.decode_u16(k) == want)
		assert_int(n).override_failure_message("no %s near the %s at %s" % [e[1], e[0], p]).is_greater(0)
