extends GdUnitTestSuite

## Multi-voxel looks (ADR 0053): crops at every age, placed flowers, mushrooms and shells, torches on
## floors and walls, the 1 m crafting table, the half-voxel animals.

var farm: Farming
var vox := {}


func before_test() -> void:
	farm = Farming.load_default()
	vox = {}
	farm.get_voxel = func(p: Vector3i) -> int: return int(vox.get(p, GameMaterials.AIR))
	farm.set_voxel = func(p: Vector3i, m: int) -> void:
		if m == GameMaterials.AIR:
			vox.erase(p)
		else:
			vox[p] = m
	farm.rng.seed = 5
	for x in range(-4, 8):
		for z in range(-4, 8):
			vox[Vector3i(x, 0, z)] = GameMaterials.GRASS


func _count_above() -> int:
	var n := 0
	for p in vox:
		if p.y > 0:
			n += 1
	return n


func test_crops_are_plants_on_a_two_voxel_grid() -> void:
	for x in 4:
		for z in 4:
			farm.till(Vector3i(x, 0, z))
	var sown := 0
	for x in 4:
		for z in 4:
			if farm.plant(Vector3i(x, 0, z), "wheat"):
				sown += 1
	assert_int(sown).is_equal(4)   # four plants to a 0.5 m block, like Minecraft's crop texture
	for p in farm.crops:
		assert_int(p.x % 2).is_equal(0)
		assert_int(p.z % 2).is_equal(0)


func test_wheat_grows_taller_and_ripens_with_ears() -> void:
	farm.till(Vector3i(0, 0, 0))
	farm.plant(Vector3i(0, 0, 0), "wheat")
	var a := Vector3i(0, 1, 0)
	var young := _count_above()
	farm.grow(a, 7)
	assert_int(_count_above()).is_greater(young + 4)
	var ears := 0
	for p in vox:
		if vox[p] == GameMaterials.id_of("wheat_ear"):
			ears += 1
	assert_int(ears).is_greater_equal(2)
	# every voxel above the farmland belongs to the plant, inside its 2 x 2 columns
	for p in vox:
		if p.y > 0:
			assert_bool(farm.parts.has(p)).is_true()
			assert_bool(p.x >= 0 and p.x < 2 and p.z >= 0 and p.z < 2).is_true()


func test_breaking_any_part_takes_the_plant_and_drops_once() -> void:
	farm.till(Vector3i(0, 0, 0))
	farm.plant(Vector3i(0, 0, 0), "carrots")
	farm.grow(Vector3i(0, 1, 0), 7)
	var leaf := Vector3i.ZERO
	for p in farm.parts:
		if p != Vector3i(0, 1, 0):
			leaf = p
			break
	assert_that(leaf).is_not_equal(Vector3i.ZERO)
	vox.erase(leaf)   # Main sets the broken voxel to air first
	var ev := farm.on_broken(leaf)
	assert_int(ev.size()).is_equal(1)
	assert_str(String(ev[0]["drops"][0]["id"])).is_equal("carrot")
	assert_int(_count_above()).is_equal(0)
	assert_bool(farm.crops.is_empty()).is_true()
	assert_bool(farm.parts.is_empty()).is_true()


func test_crop_parts_come_back_after_a_reload() -> void:
	farm.till(Vector3i(0, 0, 0))
	farm.plant(Vector3i(0, 0, 0), "wheat")
	farm.grow(Vector3i(0, 1, 0), 5)
	var parts := farm.parts.size()
	var f2 := Farming.load_default()
	assert_bool(f2.load_dict(JSON.parse_string(JSON.stringify(farm.to_dict())))).is_true()
	assert_int(f2.parts.size()).is_equal(parts)


func test_placed_flowers_mushrooms_and_shells_are_pieces() -> void:
	for name in ["oxeye_daisy", "allium", "cornflower", "flower", "flower_red"]:
		var st := PlantStamps.for_placed(GameMaterials.id_of(name), Vector3i(3, 1, 7))
		assert_int(st.size()).override_failure_message(name).is_between(20, 40)
		assert_int(st[Vector3i.ZERO]).is_equal(GameMaterials.id_of("plant_stem"))
	for x in 12:
		var st := PlantStamps.for_placed(GameMaterials.id_of("mushroom_red"), Vector3i(x * 5, 1, 0))
		assert_int(st.size()).is_between(10, 40)
		var sh := PlantStamps.for_placed(GameMaterials.id_of("seashell"), Vector3i(x * 5, 1, 0))
		assert_int(sh.size()).is_between(5, 16)
	assert_bool(PlantStamps.for_placed(GameMaterials.id_of("kelp"), Vector3i.ZERO).is_empty()).is_true()
	# one piece: a dig of any voxel takes the whole flower (ADR 0047 groups)
	var world := {}
	for o in PlantStamps.for_placed(GameMaterials.id_of("oxeye_daisy"), Vector3i.ZERO):
		world[o] = PlantStamps.for_placed(GameMaterials.id_of("oxeye_daisy"), Vector3i.ZERO)[o]
	var get := func(p: Vector3i) -> int: return int(world.get(p, 0))
	assert_int(GroundPickups.piece(get, Vector3i.ZERO).size()).is_equal(world.size())


func test_torches_stand_or_hang_and_cover_their_voxels() -> void:
	var items := ItemRegistry.load_default()
	var store := BlockEntities.new(items, RecipeRegistry.load_default(items))
	var t := store.create_torch(Vector3i(0, 1, 0), Vector3i.ZERO)
	assert_int(t.cells.size()).is_greater_equal(8)
	for c in t.cells:
		assert_object(store.at(c)).is_same(t)
	assert_float(t.flame().y).is_greater(4.0)
	var w := store.create_torch(Vector3i(10, 1, 0), Vector3i(1, 0, 0))
	for c in w.cells:
		assert_int(c.x).is_less_equal(10)   # nothing goes into the wall at x = 11
	assert_float(w.flame().x).is_less(10.0)
	# saved and loaded with its mount
	var d: Dictionary = JSON.parse_string(JSON.stringify(store.to_dict()))
	var s2 := BlockEntities.new(items, RecipeRegistry.load_default(items))
	s2.load_dict(d)
	assert_int((s2.at(Vector3i(10, 1, 0)) as TorchEntity).cells.size()).is_equal(w.cells.size())
	assert_object(s2.at(w.cells[-1])).is_not_null()
	# the torch item gives light, held or dropped
	assert_bool(TorchLights.light_of(items, "torch").is_empty()).is_false()
	assert_bool(TorchLights.light_of(items, "stone").is_empty()).is_true()


func test_crafting_table_is_a_metre_wide() -> void:
	var items := ItemRegistry.load_default()
	var store := BlockEntities.new(items, RecipeRegistry.load_default(items))
	var e := store.create(Vector3i(0, 0, 0), "station")
	assert_int(BlockEntities.footprint(e).size()).is_equal(8 * 6 * 8)
	assert_object(store.at(Vector3i(7, 5, 7))).is_same(e)
	var mesh := StationEntity.model_mesh()
	assert_object(mesh).is_not_null()
	var aabb := mesh.get_aabb()
	assert_float(aabb.size.x).is_equal_approx(8.0, 0.01)
	assert_float(aabb.end.y).is_less_equal(6.01)


func test_animals_merge_to_half_voxels_and_keep_their_eyes() -> void:
	assert_int(MobModel.cell).is_equal(2)
	for sp in ["sheep", "pig", "bear"]:
		var spec: Array = MobModel.shaded(MobModel._build(sp, "head"), sp)
		var eyes := 0
		for c in spec[0]:
			if c > 0 and (c - 1) / MobModel.SHADES == MobModel.EYE:
				eyes += 1
		assert_int(eyes).override_failure_message(sp).is_greater_equal(2)


## Bug reports 2026-11-08_16-02-53 and 16-12-57: heather and just-planted seeds are multi-voxel.
func test_heather_and_seedlings_are_multi_voxel() -> void:
	var heather := GameMaterials.id_of("heather")
	var clump := PlantStamps.for_placed(heather, Vector3i(5, 0, 9))
	assert_int(clump.size()).is_greater(8)
	assert_bool(clump.values().has(heather)).is_true()
	assert_bool(clump.values().has(GameMaterials.id_of("plant_stem"))).is_true()
	for kind in ["wheat", "emmer", "carrots"]:
		var look := GameMaterials.id_of("wheat_0" if kind != "carrots" else "carrots_0")
		assert_int(PlantStamps.crop(kind, 0, look, Vector3i(2, 0, 4)).size()).override_failure_message(kind).is_greater_equal(4)
	# emmer shares the wheat looks: it grows the wheat shape, not carrot leaves
	var emmer := PlantStamps.crop("emmer", 7, GameMaterials.id_of("wheat_7"), Vector3i(2, 0, 4))
	assert_bool(emmer.values().has(GameMaterials.id_of("carrots_0"))).is_false()
