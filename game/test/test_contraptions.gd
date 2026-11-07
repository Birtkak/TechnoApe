extends GdUnitTestSuite

## Moving contraptions (ADR 0160): driven rigid pieces (lift, drive, release), the poses ending on whole voxels,
## riding, and the drive as an LV load that a hand crank moves and the save keeps.

const B := Settings.BLOCK

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	store = BlockEntities.new(items, recipes)
	Inventory.clock_s = 1000.0


func after_test() -> void:
	Inventory.clock_s = 0.0


func _world() -> Array:
	var sim := SimWorld.new()
	sim.configure(Vector3i(4, 4, 4), 2, 1)
	sim.set_materials(GameMaterials.defs())
	sim.mark_all_loaded(true)
	sim.fill_box(Vector3i(0, 0, 0), Vector3i(64, 8, 64), GameMaterials.STONE)
	var rp := RigidPieces.new()
	rp.setup(sim, GameMaterials.defs(), GameMaterials.ATLAS_TILES)
	rp.set_origin(Vector3i.ZERO)
	return [sim, rp]


func test_a_driven_piece_moves_where_it_is_told_and_settles_there() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	sim.fill_box(Vector3i(10, 8, 10), Vector3i(22, 9, 22), GameMaterials.PLANKS)   # a 12 x 12 platform on the stone
	var r: Dictionary = rp.lift(Vector3i(15, 8, 15), Vector3i(8, 8, 8), Vector3i(30, 11, 30), 4096, false)
	assert_int(int(r.get("cells", 0))).is_equal(144)
	assert_int(sim.get_voxel(Vector3i(15, 8, 15))).is_equal(GameMaterials.AIR)   # cut out
	var id := int(r["id"])
	for k in 10:
		rp.drive(id, Transform3D(Basis(), r["origin"] + Vector3(0, k + 1, 0)))
		rp.step(1.0 / 30.0)
	assert_float(rp.get_transform(id, 1.0).origin.y).is_equal_approx(18.0, 0.001)   # no gravity: where it was driven
	assert_bool(rp.release(id, true)).is_true()
	assert_int(sim.get_voxel(Vector3i(15, 18, 15))).is_equal(GameMaterials.PLANKS)
	assert_int(rp.get_ids().size()).is_equal(0)


func test_a_part_running_past_its_box_is_not_taken() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	sim.fill_box(Vector3i(10, 8, 10), Vector3i(40, 9, 12), GameMaterials.PLANKS)   # a floor going on past the box
	var r: Dictionary = rp.lift(Vector3i(15, 8, 11), Vector3i(8, 8, 8), Vector3i(30, 11, 30), 4096, false)
	assert_str(String(r.get("error", ""))).is_equal("edge")
	assert_int(sim.get_voxel(Vector3i(15, 8, 11))).is_equal(GameMaterials.PLANKS)   # left alone
	assert_int(int(rp.lift(Vector3i(15, 8, 11), Vector3i(8, 8, 8), Vector3i(45, 11, 30), 4096, true).get("cells", 0))).is_equal(30 * 2)   # inside a bigger box; planks are wood: crane cargo
	assert_str(String(rp.lift(Vector3i(15, 2, 11), Vector3i(8, 0, 8), Vector3i(30, 11, 30), 4096, true).get("error", ""))).is_equal("nothing")   # stone is no cargo


func test_a_released_crane_load_falls_and_settles() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	sim.fill_box(Vector3i(20, 8, 20), Vector3i(36, 12, 24), GameMaterials.WOOD)   # a log on the ground
	var r: Dictionary = rp.lift(Vector3i(25, 9, 21), Vector3i(15, 0, 15), Vector3i(45, 20, 30), 4096, true)
	var id := int(r["id"])
	rp.drive(id, Transform3D(Basis(), r["origin"] + Vector3(0, 6, 0)))
	rp.step(1.0 / 30.0)
	rp.release(id, false)
	var done := false
	for t in 30 * 10:
		if rp.step(1.0 / 30.0).has(id):
			done = true
			break
	assert_bool(done).is_true()
	assert_int(sim.get_voxel(Vector3i(25, 9, 21))).is_equal(GameMaterials.WOOD)   # back on the ground


func test_poses_end_on_whole_voxels() -> void:
	for kind in ContraptionEntity.KINDS:
		for f in 4:
			var e := ContraptionEntity.new(items, Vector3i(40, 16, 40), kind)
			e.facing = f
			var t := Contraptions.pose_of(e, Contraptions.end_of(e) if kind == "lift_winch" else 90.0)
			for p in [Vector3(41, 16, 47), Vector3(45, 16, 60), Vector3(44, 21, 44)]:
				var q: Vector3 = t * p
				assert_vector(q).override_failure_message("%s facing %d: %s -> %s" % [kind, f, p, q]).is_equal_approx(q.round(), Vector3.ONE * 0.001)


func test_the_drawbridge_raises_its_far_end_in_front_of_the_hinge() -> void:
	var e := ContraptionEntity.new(items, Vector3i(0, 0, 0), "drawbridge_hinge")
	e.facing = 0
	var f := Vector3(Transport.dir_of(0)) / B
	var far := Vector3(B * 0.5, 0.5, B * 0.5) + f * (B * 0.5 + 30.0)   # a span cell 30 voxels out
	var up: Vector3 = Contraptions.pose_of(e, 90.0) * far
	assert_float(up.y).is_greater(30.0)
	assert_float((up - Vector3(B * 0.5, 0, B * 0.5)).dot(f)).is_between(B * 0.5, B * 1.5)   # within a block in front of the hinge


func test_a_rider_follows_the_top_and_a_steep_part_throws_them_off() -> void:
	var size := Vector3i(12, 1, 12)
	var a := Transform3D(Basis(), Vector3(10, 8, 10))
	var b := Transform3D(Basis(), Vector3(10, 9, 10))
	var feet := Vector3(16, 9.2, 16)
	assert_bool(Contraptions._rides(a, size, feet)).is_true()
	assert_vector(Contraptions._carried(a, b, size, feet)).is_equal_approx(Vector3(16, 10, 16), Vector3.ONE * 0.001)
	assert_bool(Contraptions._rides(a, size, Vector3(30, 9, 16))).is_false()   # beside it
	assert_bool(Contraptions._rides(a, size, Vector3(16, 14, 16))).is_false()  # well above it
	var steep := Transform3D(Basis(Vector3.RIGHT, deg_to_rad(60.0)), Vector3(10, 8, 10))
	assert_bool(Contraptions._rides(steep, size, steep * Vector3(6, 1.2, 6))).is_false()
	assert_bool(Contraptions._overlaps(b, size, Vector3(16, 6, 16), AABB(Vector3(-2, 0, -2), Vector3(4, 14, 4)))).is_true()   # stood under it


func test_a_crank_moves_the_lever_and_the_drive_saves() -> void:
	var drive: ContraptionEntity = store.create(Vector3i(0, 0, 0), "lift_winch")
	var crank: PowerSourceEntity = store.create(Vector3i(B, 0, 0), "hand_crank")
	crank.cranked_until = Inventory.clock_s + 1000.0
	for i in 60:
		store.tick()
	assert_bool(drive.load.on).override_failure_message("at its end: draws nothing").is_false()
	drive.use({})
	assert_bool(drive.lever).is_true()
	for i in 60:
		store.tick()
	assert_bool(drive.load.on).override_failure_message(drive.load.status()).is_true()
	drive.pose = 12.0
	drive.facing = 3
	var store2 := BlockEntities.new(items, recipes)
	assert_bool(store2.load_dict(store.to_dict())).is_true()
	var d2: ContraptionEntity = store2.at(Vector3i(0, 0, 0))
	assert_bool(d2.lever).is_true()
	assert_float(d2.pose).is_equal(12.0)
	assert_int(d2.facing).is_equal(3)


func test_every_drive_is_craftable_with_a_sprite() -> void:
	for kind in ContraptionEntity.KINDS:
		assert_str(String(items.get_def(kind).get("work", ""))).is_equal(kind)
		assert_bool(recipes.makes(kind).is_empty()).override_failure_message(kind + " has no recipe").is_false()
