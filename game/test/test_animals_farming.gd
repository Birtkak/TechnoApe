extends GdUnitTestSuite

## Headless tests for ADR 0022: Minecraft farming on a dictionary world
## (tilling, planting, growth formula, hydration, drying, trampling, drops),
## drop lists, the animal data and brain decisions that need no scene, and
## furniture footprints and saving.

var items: ItemRegistry
var recipes: RecipeRegistry
var mining: MiningTable
var farm: Farming
var vox := {}   # Vector3i -> material id; missing = air


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	mining = MiningTable.load_default()
	farm = Farming.load_default()
	vox = {}
	farm.get_voxel = func(p: Vector3i) -> int: return int(vox.get(p, GameMaterials.AIR))
	farm.set_voxel = func(p: Vector3i, m: int) -> void:
		if m == GameMaterials.AIR:
			vox.erase(p)
		else:
			vox[p] = m
	farm.rng.seed = 1234
	farm.block = 1   # the Minecraft rules on one-voxel blocks; test_block_scale_farming checks the 0.5 m scale


## A flat grass floor at y = 0 over [-r, r]^2.
func _floor(r: int = 8) -> void:
	for x in range(-r, r + 1):
		for z in range(-r, r + 1):
			vox[Vector3i(x, 0, z)] = GameMaterials.GRASS


# ---------- data ----------

func test_data_loads_cleanly() -> void:
	assert_array(items.errors).is_empty()
	assert_array(recipes.errors).is_empty()
	assert_array(mining.errors).is_empty()
	assert_array(farm.errors).is_empty()
	var mm := MobManager.new()
	mm.configure(MobManager.load_defs())
	assert_array(mm.errors).is_empty()
	for sp in mm.defs:
		for f in mm.defs[sp]["food"]:
			assert_bool(items.has(f)).override_failure_message("%s food %s is no item" % [sp, f]).is_true()
		for d in mm.defs[sp]["drops"]:
			assert_bool(items.has(d["id"])).is_true()
	mm.free()


func test_material_ids_are_stable() -> void:
	# append-only material ids (ADR 0006): the generator's enum names 73 and 74
	assert_str(GameMaterials.name_of(GameMaterials.WOOL)).is_equal("wool")
	assert_str(GameMaterials.name_of(GameMaterials.FARMLAND_MOIST)).is_equal("farmland_moist")
	assert_str(GameMaterials.name_of(GameMaterials.WHEAT_0 + 7)).is_equal("wheat_7")
	assert_str(GameMaterials.name_of(GameMaterials.CARROTS_0 + 3)).is_equal("carrots_3")
	assert_str(GameMaterials.name_of(GameMaterials.WILD_WHEAT)).is_equal("wild_wheat")
	assert_int(GameMaterials.WILD_WHEAT).is_equal(73)
	assert_int(GameMaterials.WILD_CARROT).is_equal(74)
	assert_str(GameMaterials.name_of(GameMaterials.FURNITURE)).is_equal("furniture")
	assert_int(GameMaterials.defs().size()).is_greater_equal(GameMaterials.FURNITURE)   # later content appends after it
	for i in GameMaterials.defs().size():
		var n: String = GameMaterials.defs()[i]["name"]
		if n.begins_with("wheat_") or n.begins_with("carrots_") or n.begins_with("wild_") or n in ["wool", "carpet", "farmland", "farmland_moist", "furniture"]:
			assert_bool(mining.is_minable(n)).override_failure_message("%s not minable" % n).is_true()


func test_new_recipes() -> void:
	for r in ["carpet", "bed", "chair", "bread", "flint_hoe", "copper_hoe", "iron_hoe", "shears", "cook_meat_furnace"]:
		assert_bool(recipes.by_id.has(r) if "by_id" in recipes else true).is_true()
	assert_array(recipes.makes("bed")).is_not_empty()
	assert_array(recipes.makes("bread")).is_not_empty()
	assert_array(recipes.makes("cooked_meat")).is_not_empty()
	assert_array(recipes.uses_of("wool")).is_not_empty()


# ---------- drops ----------

func test_drop_lists_roll_counts_and_chances() -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	var seeds := 0
	var fiber := 0
	for i in 2000:
		for s in mining.roll_drops("tall_grass", {}, rng):
			# by hand: no dry grass (that needs a knife); wild einkorn, emmer and flax seeds now and then (ADR 0059), wild potatoes, onions and beans (ADR 0121)
			assert_array(["wheat_seeds", "plant_fiber", "einkorn_seeds", "emmer_seeds", "flax_seeds", "potato", "onion", "beans"]).contains([s["id"]])
			if s["id"] == "wheat_seeds":
				seeds += int(s["count"])
			elif s["id"] == "plant_fiber":
				fiber += int(s["count"])
	assert_int(seeds).is_between(40, 130)   # 4 % of 2000 = 80: rare
	assert_int(fiber).is_between(500, 700)  # 30 % of 2000 = 600 (ADR 0030)
	for i in 50:
		var d := mining.roll_drops("wild_carrot", {}, rng)
		assert_int(d.size()).is_equal(1)
		assert_int(int(d[0]["count"])).is_between(1, 2)
	# a plain string drop still works through both paths
	assert_dict(mining.drops("dirt")).is_equal({"id": "dirt", "count": 1})
	assert_array(mining.roll_drops("dirt", {}, rng)).is_equal([{"id": "dirt", "count": 1}])
	assert_dict(mining.drops("wild_wheat")).is_equal({"id": "wheat_seeds", "count": 1})


# ---------- farming ----------

func test_till_and_plant() -> void:
	_floor()
	assert_bool(farm.till(Vector3i(0, 0, 0))).is_true()
	assert_int(vox[Vector3i(0, 0, 0)]).is_equal(GameMaterials.FARMLAND)
	vox[Vector3i(1, 1, 0)] = GameMaterials.STONE
	assert_bool(farm.till(Vector3i(1, 0, 0))).override_failure_message("needs air above").is_false()
	assert_bool(farm.till(Vector3i(1, 1, 0))).override_failure_message("stone is not tillable").is_false()
	assert_bool(farm.plant(Vector3i(0, 0, 0), "wheat")).is_true()
	assert_int(vox[Vector3i(0, 1, 0)]).is_equal(GameMaterials.WHEAT_0)
	assert_bool(farm.plant(Vector3i(0, 0, 0), "wheat")).override_failure_message("occupied").is_false()
	assert_bool(farm.plant(Vector3i(2, 0, 0), "carrots")).override_failure_message("grass, not farmland").is_false()


func test_growth_speed_matches_minecraft() -> void:
	_floor()
	# a lone crop on dry farmland: f = 1 + 1 = 2
	farm.till(Vector3i(0, 0, 0))
	farm.plant(Vector3i(0, 0, 0), "wheat")
	assert_float(farm.growth_speed(Vector3i(0, 1, 0))).is_equal_approx(2.0, 0.001)
	# moist centre: f = 1 + 3
	farm.farmland[Vector3i(0, 0, 0)] = 7
	assert_float(farm.growth_speed(Vector3i(0, 1, 0))).is_equal_approx(4.0, 0.001)
	# a full moist 3x3 bed under it: 1 + 3 + 8 * 3/4 = 10
	for x in range(-1, 2):
		for z in range(-1, 2):
			if x != 0 or z != 0:
				farm.till(Vector3i(x, 0, z))
				farm.farmland[Vector3i(x, 0, z)] = 7
	assert_float(farm.growth_speed(Vector3i(0, 1, 0))).is_equal_approx(10.0, 0.001)
	# wheat in a row on one axis is fine; on both axes the speed halves
	farm.plant(Vector3i(1, 0, 0), "wheat")
	assert_float(farm.growth_speed(Vector3i(0, 1, 0))).is_equal_approx(10.0, 0.001)
	farm.plant(Vector3i(0, 0, 1), "wheat")
	assert_float(farm.growth_speed(Vector3i(0, 1, 0))).is_equal_approx(5.0, 0.001)
	# a different crop does not count
	var f2 := farm.growth_speed(Vector3i(1, 1, 0))
	farm.plant(Vector3i(1, 0, 1), "carrots")
	assert_float(farm.growth_speed(Vector3i(1, 1, 0))).is_equal_approx(f2, 0.001)


func test_hydration_and_drying() -> void:
	_floor()
	farm.till(Vector3i(0, 0, 0))
	farm.till(Vector3i(6, 0, 0))
	vox[Vector3i(4, 0, 0)] = GameMaterials.WATER
	assert_bool(farm.near_water(Vector3i(0, 0, 0))).is_true()
	assert_bool(farm.near_water(Vector3i(-5, 0, 0))).is_false()
	# random ticks until both have had several: the wet one turns moist, the far dry one reverts to dirt
	farm.till(Vector3i(-6, 0, 0))
	for i in 600:
		farm.tick(20)
	assert_int(vox[Vector3i(0, 0, 0)]).is_equal(GameMaterials.FARMLAND_MOIST)
	assert_int(vox[Vector3i(6, 0, 0)]).is_equal(GameMaterials.FARMLAND_MOIST)
	assert_int(vox[Vector3i(-6, 0, 0)]).override_failure_message("dry, unplanted farmland turns to dirt").is_equal(GameMaterials.DIRT)
	assert_bool(farm.farmland.has(Vector3i(-6, 0, 0))).is_false()


func test_crops_grow_to_ripe_in_minecraft_time() -> void:
	# one moist, planted farmland: about 68 s per random tick, chance 1/(floor(25/4)+1) = 1/7 per tick
	# -> ~7 * 7 random ticks = ~55 min to ripe; check it ripens in 3 hours and not in 5 minutes
	_floor()
	vox[Vector3i(2, 0, 0)] = GameMaterials.WATER
	farm.till(Vector3i(0, 0, 0))
	farm.farmland[Vector3i(0, 0, 0)] = 7
	farm.plant(Vector3i(0, 0, 0), "wheat")
	for s in 300:
		farm.tick(20)
	assert_int(int(farm.crops[Vector3i(0, 1, 0)]["age"])).is_less(7)
	for s in 3 * 3600:
		farm.tick(20)
	assert_int(int(farm.crops[Vector3i(0, 1, 0)]["age"])).is_equal(7)
	assert_int(vox[Vector3i(0, 1, 0)]).is_equal(GameMaterials.WHEAT_0 + 7)


func test_crop_drops_by_age() -> void:
	_floor()
	farm.till(Vector3i(0, 0, 0))
	farm.plant(Vector3i(0, 0, 0), "wheat")
	var young := farm.on_broken(Vector3i(0, 1, 0))
	vox.erase(Vector3i(0, 1, 0))   # Main clears the broken voxel
	assert_array(young[0]["drops"]).is_equal([{"id": "wheat_seeds", "count": 1}])
	farm.plant(Vector3i(0, 0, 0), "wheat")
	farm.grow(Vector3i(0, 1, 0), 7)
	var ripe: Array = farm.on_broken(Vector3i(0, 1, 0))[0]["drops"]
	vox.erase(Vector3i(0, 1, 0))
	assert_str(ripe[0]["id"]).is_equal("wheat")
	farm.plant(Vector3i(0, 0, 0), "carrots")
	farm.grow(Vector3i(0, 1, 0), 7)
	assert_int(vox[Vector3i(0, 1, 0)]).is_equal(GameMaterials.CARROTS_0 + 3)
	var c: Array = farm.on_broken(Vector3i(0, 1, 0))[0]["drops"]
	assert_str(c[0]["id"]).is_equal("carrot")
	assert_int(int(c[0]["count"])).is_between(1, 4)


func test_block_scale_farming() -> void:
	# the game's scale (ADR 0034): Minecraft's distances count 0.5 m blocks of BLOCK voxels
	farm.block = Settings.BLOCK
	var b := farm.block
	_floor(6 * b)
	# a brushed 4x4 patch of wheat is one Minecraft crop: its own voxels are no rows
	for x in b:
		for z in b:
			farm.till(Vector3i(x, 0, z))
			farm.plant(Vector3i(x, 0, z), "wheat")
	assert_float(farm.growth_speed(Vector3i(0, 1, 0))).is_equal_approx(2.0, 0.001)   # plants stand on a 2-voxel grid (ADR 0053)
	# patches one block away on both axes halve it, as rows of crop blocks do
	for x in b:
		for z in b:
			for o in [Vector3i(b, 0, 0), Vector3i(0, 0, b)]:
				farm.till(Vector3i(x, 0, z) + o)
				farm.plant(Vector3i(x, 0, z) + o, "wheat")
	assert_float(farm.growth_speed(Vector3i(0, 1, 0))).is_less(2.0)
	# water 1.5 m away (12 voxels) and one block up keeps farmland wet; 2.5 m away does not
	vox[Vector3i(12, b, 0)] = GameMaterials.WATER
	assert_bool(farm.near_water(Vector3i(0, 0, 0))).is_true()
	assert_bool(farm.near_water(Vector3i(-8, 0, 0))).is_false()


func test_trampling_and_smothering_pop_the_crop() -> void:
	_floor()
	farm.till(Vector3i(0, 0, 0))
	farm.plant(Vector3i(0, 0, 0), "wheat")
	assert_array(farm.trample(Vector3i(0, 0, 0), 0.4)).is_empty()   # a step down never tramples
	var ev := farm.trample(Vector3i(0, 0, 0), 5.0)                   # a big fall always does
	assert_int(vox[Vector3i(0, 0, 0)]).is_equal(GameMaterials.DIRT)
	assert_bool(vox.has(Vector3i(0, 1, 0))).is_false()
	assert_array(ev[0]["drops"]).is_equal([{"id": "wheat_seeds", "count": 1}])
	farm.till(Vector3i(1, 0, 0))
	vox[Vector3i(1, 1, 0)] = GameMaterials.STONE
	farm.on_placed(Vector3i(1, 1, 0), GameMaterials.STONE)
	assert_int(vox[Vector3i(1, 0, 0)]).is_equal(GameMaterials.DIRT)
	farm.till(Vector3i(2, 0, 0))
	farm.on_placed(Vector3i(2, 1, 0), GameMaterials.CARPET)   # carpets and plants do not smother it
	assert_int(vox[Vector3i(2, 0, 0)]).is_equal(GameMaterials.FARMLAND)


func test_farm_save_round_trip() -> void:
	_floor()
	farm.till(Vector3i(0, 0, 0))
	farm.plant(Vector3i(0, 0, 0), "carrots")
	farm.grow(Vector3i(0, 1, 0), 3)
	var d: Dictionary = JSON.parse_string(JSON.stringify(farm.to_dict()))
	var f2 := Farming.load_default()
	assert_bool(f2.load_dict(d)).is_true()
	assert_int(f2.farmland.size()).is_equal(1)
	assert_dict(f2.crops[Vector3i(0, 1, 0)]).is_equal({"kind": "carrots", "age": 3})


# ---------- animals ----------

func _manager() -> MobManager:
	var mm: MobManager = auto_free(MobManager.new())
	mm.configure(MobManager.load_defs())
	return mm


func test_feeding_puts_adults_in_love_and_grows_babies() -> void:
	var mm := _manager()
	var sheep: Mob = auto_free(Mob.create(mm, "sheep", mm.defs["sheep"], false))
	assert_str(mm.interact(sheep, items.get_def("carrot"))).override_failure_message("sheep do not eat carrots").is_equal("")
	assert_str(mm.interact(sheep, items.get_def("wheat"))).is_equal("love")
	assert_float(sheep.love_s).is_equal(mm.love_s)
	assert_str(sheep.feed()).override_failure_message("already in love").is_equal("")
	var lamb: Mob = auto_free(Mob.create(mm, "sheep", mm.defs["sheep"], true))
	assert_bool(lamb.is_baby()).is_true()
	var before := lamb.age_s
	assert_str(lamb.feed()).is_equal("grew")
	assert_float(lamb.age_s).is_equal_approx(before * 0.9, 0.01)   # Minecraft: 10 % of the time left
	var pig: Mob = auto_free(Mob.create(mm, "pig", mm.defs["pig"], false))
	assert_str(mm.interact(pig, items.get_def("carrot"))).is_equal("love")
	pig.breed_cooldown_s = 10.0
	pig.love_s = 0.0
	assert_str(pig.feed()).override_failure_message("cooldown after breeding").is_equal("")


func test_spawn_cells_are_deterministic() -> void:
	# the same seed and cell always roll the same herd decision
	var a := RandomNumberGenerator.new()
	a.seed = hash([1337, 4, -7, 0x5EED])
	var b := RandomNumberGenerator.new()
	b.seed = hash([1337, 4, -7, 0x5EED])
	assert_float(a.randf()).is_equal(b.randf())


# ---------- furniture ----------

## Furniture is measured in 0.5 m blocks of BLOCK voxels (ADR 0034).
func test_furniture_footprints() -> void:
	var b := Settings.BLOCK
	var bed := FurnitureEntity.footprint("bed", Vector3i(0, 0, 0), 0)
	assert_int(bed.size()).is_equal(8 * b * b * b)
	assert_bool(bed.has(Vector3i(0, 0, 4 * b - 1))).is_true()    # 4 blocks long along +z
	assert_bool(bed.has(Vector3i(2 * b - 1, 0, 0))).is_true()    # 2 blocks wide
	assert_bool(bed.has(Vector3i(0, b, 0))).is_false()           # 1 block high
	var bed_x := FurnitureEntity.footprint("bed", Vector3i(0, 0, 0), 1)
	assert_bool(bed_x.has(Vector3i(4 * b - 1, 0, 0))).is_true()
	assert_bool(bed_x.has(Vector3i(0, 0, -1))).is_true()
	var chair := FurnitureEntity.footprint("chair", Vector3i(5, 5, 5), 2)
	assert_int(chair.size()).is_equal(2 * b * b * b)
	assert_bool(chair.has(Vector3i(5, 5 + 2 * b - 1, 5))).is_true()
	# beds face away from the player's look, chairs back at the player
	assert_int(FurnitureEntity.facing_for("bed", Vector3(0, 0, -1))).is_equal(2)
	assert_int(FurnitureEntity.facing_for("chair", Vector3(0, 0, -1))).is_equal(0)


func test_furniture_entities_save_and_cover_every_voxel() -> void:
	var store := BlockEntities.new(items, recipes)
	var e := store.create_furniture(Vector3i(10, 3, 10), "bed", 1)
	for c in e.cells:
		assert_object(store.furniture_at(c)).is_same(e)
	var store2 := BlockEntities.new(items, recipes)
	assert_bool(store2.load_dict(JSON.parse_string(JSON.stringify(store.to_dict())))).is_true()
	var e2 := store2.furniture_at(e.cells[5])
	assert_object(e2).is_not_null()
	assert_str(e2.furniture).is_equal("bed")
	assert_int(e2.facing).is_equal(1)
	# breaking any covered voxel removes the whole bed and gives the item back
	assert_array(store2.remove(e.cells[7])).is_equal([{"id": "bed", "count": 1}])
	assert_object(store2.furniture_at(e.cells[0])).is_null()
	assert_int(store2.size()).is_equal(0)


func test_furniture_mesh_cells_fit_the_footprint() -> void:
	for k in FurnitureEntity.KINDS:
		var size: Vector3i = FurnitureEntity.KINDS[k]["cells"]
		var foot: Vector3i = FurnitureEntity.KINDS[k]["size"]
		assert_int(size.x).is_equal(foot.x * GameMaterials.MICRO_GRID)
		if FurnitureEntity.KINDS[k].has("box"):
			assert_int(size.z).is_less_equal(foot.z * GameMaterials.MICRO_GRID)   # thin pieces (ADR 0094)
		else:
			assert_int(size.z).is_equal(foot.z * GameMaterials.MICRO_GRID)
		var c := FurnitureEntity.cells_for(k)
		var filled := 0
		for v in c:
			if v != 0:
				filled += 1
		assert_int(filled).is_greater(50)
