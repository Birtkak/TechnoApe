extends GdUnitTestSuite

## P12 rigid voxel pieces (RigidPieces, prototypes/p12-rigid-pieces): what breaks off, and that it
## comes back as voxels.

const N := Vector3i(4, 4, 4)   # 8 m cube of 12.5 cm voxels


func _world() -> Array:
	var sim := SimWorld.new()
	sim.configure(N, 2, 1)
	sim.set_materials(GameMaterials.defs())
	sim.mark_all_loaded(true)
	sim.fill_box(Vector3i(0, 0, 0), Vector3i(64, 8, 64), GameMaterials.STONE)   # the ground: y < 8
	var rp := RigidPieces.new()
	rp.setup(sim, GameMaterials.defs(), GameMaterials.ATLAS_TILES)
	rp.set_origin(Vector3i.ZERO)
	return [sim, rp]


## A 4x4 log pillar from the ground to y = 40; returns the cells of a cut through it at `cut_y`.
func _pillar(sim: SimWorld, cut_y: int, cut: bool) -> Array:
	sim.fill_box(Vector3i(30, 8, 30), Vector3i(34, 40, 34), GameMaterials.WOOD)
	var cells := []
	if cut:
		for z in range(30, 34):
			for x in range(30, 34):
				sim.set_voxel(Vector3i(x, cut_y, z), GameMaterials.AIR)
				cells.append(Vector3i(x, cut_y, z))
	return cells


func test_a_cut_pillar_breaks_off() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	var ids := rp.check(_pillar(sim, 12, true), Vector3(0, 12, 32))
	assert_int(ids.size()).is_equal(1)
	assert_int(int(rp.get_info(ids[0])["wood"])).is_equal(16 * 27)   # y 13..39 above the cut
	assert_int(sim.get_voxel(Vector3i(31, 20, 31))).is_equal(GameMaterials.AIR)   # left the grid


func test_a_pillar_on_the_ground_stays() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	_pillar(sim, 12, false)
	# a notch in one side: still held by the other 12 columns, and strong enough for 27 layers
	var notch := []
	for x in range(30, 34):
		sim.set_voxel(Vector3i(x, 12, 30), GameMaterials.AIR)
		notch.append(Vector3i(x, 12, 30))
	assert_int(rp.check(notch, Vector3(0, 12, 32)).size()).is_equal(0)


func test_a_dig_in_the_ground_is_free() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	sim.set_voxel(Vector3i(10, 7, 10), GameMaterials.AIR)
	assert_int(rp.check([Vector3i(10, 7, 10)], Vector3.ZERO).size()).is_equal(0)


func test_a_fallen_piece_settles_back_into_voxels() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	var ids := rp.check(_pillar(sim, 12, true), Vector3(0, 12, 32))
	var settled := PackedInt32Array()
	for t in 30 * 20:
		settled = rp.step(1.0 / 30.0)
		if not settled.is_empty():
			break
	assert_int(settled.size()).is_equal(1)
	assert_int(settled[0]).is_equal(ids[0])
	var wood := sim.count_material(GameMaterials.WOOD)
	# the stump (16 x 4 below the cut) plus nearly all of the 432 that fell
	assert_int(wood).is_greater(16 * 4 + 400)
	assert_int(wood).is_less_equal(16 * 4 + 432)
	# it lies down: nothing of it stands above 3 m any more
	assert_int(sim.count_in_box(Vector3i(0, 24, 0), Vector3i(64, 64, 64), GameMaterials.WOOD)).is_equal(0)
