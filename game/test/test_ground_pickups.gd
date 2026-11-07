extends GdUnitTestSuite

## Ground pickups go whole (ADR 0043): a stick or a nodule of stones is the touching voxels of one
## pickup material; a dig that touches any of them takes all of them, one piece at a time. And the
## firepit's dry run, which decides whether a right click on it is held.

var voxels := {}


func _voxel(p: Vector3i) -> int:
	return voxels.get(p, GameMaterials.AIR)


func _stick(from: Vector3i, n: int) -> void:
	for x in n:
		voxels[from + Vector3i(x, 0, 0)] = GameMaterials.TWIGS


func before_test() -> void:
	voxels.clear()


func test_a_piece_is_the_touching_voxels_of_one_material() -> void:
	_stick(Vector3i(0, 0, 0), 10)
	voxels[Vector3i(4, 1, 1)] = GameMaterials.TWIGS        # touches diagonally: same stick
	voxels[Vector3i(10, 0, 0)] = GameMaterials.FLINT_STONES  # another material: not part of it
	_stick(Vector3i(0, 0, 3), 4)                            # a gap of two: another stick
	var pc := GroundPickups.piece(_voxel, Vector3i(7, 0, 0))
	assert_int(pc.size()).is_equal(11)
	assert_object(pc[0]).is_equal(Vector3i(7, 0, 0))
	assert_bool(Vector3i(10, 0, 0) in pc).is_false()
	assert_array(GroundPickups.piece(_voxel, Vector3i(0, 5, 0))).is_empty()


func test_a_dig_completes_every_piece_it_touches() -> void:
	_stick(Vector3i(0, 0, 0), 10)
	_stick(Vector3i(0, 0, 3), 4)
	var dug: Array[Vector3i] = [Vector3i(9, 0, 0), Vector3i(9, -1, 0), Vector3i(0, 0, 3), Vector3i(1, 0, 3)]
	var r := GroundPickups.complete(_voxel, dug)
	assert_int(r["cells"].size()).is_equal(10 + 1 + 4)       # both sticks whole, the dirt cell once
	assert_int(r["pieces"].size()).is_equal(2)
	assert_int(r["pieces"][0]["cells"].size()).is_equal(10)
	assert_float((r["pieces"][0]["center"] as Vector3).x).is_equal_approx(5.0, 1e-4)


func test_a_piece_is_capped() -> void:
	for x in 40:
		for z in 4:
			voxels[Vector3i(x, 0, z)] = GameMaterials.PEBBLES
	assert_int(GroundPickups.piece(_voxel, Vector3i(0, 0, 0)).size()).is_less_equal(GroundPickups.MAX_CELLS)


func test_firepit_dry_run_changes_nothing() -> void:
	var items := ItemRegistry.load_default()
	var recipes := RecipeRegistry.load_default(items)
	var store := BlockEntities.new(items, recipes)
	var f := store.create(Vector3i(0, 10, 0), "firepit") as FirepitEntity
	assert_bool(f.use({}, {}, true)["act"]).is_false()   # nothing to take off: an instant message instead
	var log_def := items.get_def("oak_log")
	assert_bool(f.use({"id": "oak_log", "count": 64}, log_def, true)["act"]).is_true()
	assert_bool(f.inv.is_empty_slot(FirepitEntity.FUEL)).is_true()
	f.use({"id": "oak_log", "count": 64}, log_def)
	assert_bool(f.use({}, {}, true)["act"]).is_true()
	assert_bool(f.use({"id": "dirt", "count": 64}, items.get_def("dirt"), true)["act"]).is_false()
	assert_bool(f.use({"id": "firestarter", "count": 1}, items.get_def("firestarter"), true)["act"]).is_true()
	assert_bool(f.is_lit()).is_false()


# ---------- clutter and nuggets (ADR 0047) ----------

func test_a_flower_is_one_piece_and_drops_as_its_petals() -> void:
	for y in 4:
		voxels[Vector3i(0, y, 0)] = GameMaterials.PLANT_STEM
	for x in range(-1, 2):
		for z in range(-1, 2):
			voxels[Vector3i(x, 4, z)] = GameMaterials.ALLIUM
	var dug: Array[Vector3i] = [Vector3i(0, 1, 0)]   # the stem
	var r := GroundPickups.complete(_voxel, dug)
	assert_int(r["cells"].size()).is_equal(4 + 9)
	assert_int(r["pieces"].size()).is_equal(1)
	assert_int(r["pieces"][0]["material"]).is_equal(GameMaterials.ALLIUM)
	assert_bool(r["pieces"][0]["per_voxel"]).is_false()
	assert_bool(GroundPickups.is_pickup(GameMaterials.ALLIUM)).is_true()   # popped by a held right click


func test_a_glow_mushroom_is_one_piece_and_drops_as_its_cap() -> void:
	# bug report 2026-11-07 10-27-47: a stem under a glowing cap, dug whole like a flower
	voxels[Vector3i(0, 0, 0)] = GameMaterials.MUSHROOM_STEM
	voxels[Vector3i(0, 1, 0)] = GameMaterials.GLOW_MUSHROOM
	voxels[Vector3i(0, 2, 0)] = GameMaterials.GLOW_MUSHROOM
	var dug: Array[Vector3i] = [Vector3i(0, 2, 0)]
	var r := GroundPickups.complete(_voxel, dug)
	assert_int(r["cells"].size()).is_equal(3)
	assert_int(r["pieces"].size()).is_equal(1)
	assert_int(r["pieces"][0]["material"]).is_equal(GameMaterials.GLOW_MUSHROOM)
	assert_bool(GroundPickups.is_pickup(GameMaterials.GLOW_MUSHROOM)).is_true()
	assert_bool(GameMaterials.blocks_bodies(GameMaterials.GLOW_MUSHROOM)).is_false()


func test_stalks_go_one_at_a_time_and_drop_per_voxel() -> void:
	for y in 10:
		voxels[Vector3i(0, y, 0)] = GameMaterials.KELP
		voxels[Vector3i(1, y, 0)] = GameMaterials.KELP   # the stalk beside it stays
	var pc := GroundPickups.piece(_voxel, Vector3i(0, 4, 0))
	assert_int(pc.size()).is_equal(10)
	assert_bool(Vector3i(1, 4, 0) in pc).is_false()
	assert_bool(GroundPickups.drops_per_voxel(GameMaterials.KELP)).is_true()
	assert_bool(GroundPickups.is_pickup(GameMaterials.KELP)).is_false()
	assert_bool(GameMaterials.is_waterlogged(GameMaterials.KELP)).is_true()


func test_an_ore_nugget_digs_whole_and_yields_its_voxels() -> void:
	for x in 3:
		for y in 3:
			for z in 3:
				voxels[Vector3i(x, y, z)] = GameMaterials.id_of("iron_ore")
	voxels[Vector3i(5, 0, 0)] = GameMaterials.id_of("iron_ore")   # the next nugget, a voxel of stone apart
	var dug: Array[Vector3i] = [Vector3i(1, 1, 1)]
	var r := GroundPickups.complete(_voxel, dug)
	assert_int(r["cells"].size()).is_equal(27)
	assert_bool(r["pieces"][0]["per_voxel"]).is_true()
	assert_bool(GroundPickups.is_pickup(GameMaterials.id_of("iron_ore"))).is_false()   # a pickaxe job, not a hand pick
