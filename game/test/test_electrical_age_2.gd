extends GdUnitTestSuite

## The electrical age, batch 2 (ADR 0117): the pump drawing water through the bucket's scoop over a fake
## world and filling a tank, the coke oven multiblock forming on a hollow 3 x 3 x 3 of bricks and coking
## coal without power, machine pins picking what needs the simulation, saves.

const B := Settings.BLOCK

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities
var world := {}   # Vector3i -> material: a fake world for the voxel callables


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	store = BlockEntities.new(items, recipes)
	world = {}
	Inventory.clock_s = 1000.0


func after_test() -> void:
	Inventory.clock_s = 0.0


func _gv(p: Vector3i) -> int:
	return int(world.get(p, GameMaterials.AIR))


func _sv(p: Vector3i, m: int) -> void:
	world[p] = m


func _fill(lo: Vector3i, size: Vector3i, m: int) -> void:
	for z in size.z:
		for y in size.y:
			for x in size.x:
				world[lo + Vector3i(x, y, z)] = m


func test_items_and_recipes_exist() -> void:
	for id in ["pump", "coke_oven", "coal_coke"]:
		assert_bool(items.has(id)).override_failure_message(id).is_true()
		assert_bool(recipes.recipes.has(id) or id == "coal_coke").override_failure_message("recipe " + id).is_true()
	for id in ["coke_coal", "coke_logs", "alloy_steel_coke"]:
		assert_bool(recipes.recipes.has(id)).override_failure_message(id).is_true()
	assert_int(int(items.get_def("coal_coke")["fuel_ticks"])).is_equal(3200)


func test_pump_draws_a_bucket_of_connected_water_into_a_tank() -> void:
	var pump: PumpEntity = store.create(Vector3i(0, 20, 0), "pump")
	var tank: TankEntity = store.create(Vector3i(2 * B, 20, 0), "tank")   # the pump is 2 blocks wide
	_fill(Vector3i(-8, 10, -8), Vector3i(20, 10, 20), GameMaterials.WATER)   # 4000 cells of water under it
	var below := pump.pos + Vector3i(B / 2, -1, B / 2)
	assert_bool(pump.draw_from(_gv, _sv, below)).is_true()   # the edit itself; WorkSystem calls it only while wants_water()
	assert_int(pump.buckets).is_equal(1)
	var water := 0
	for p in world:
		if world[p] == GameMaterials.WATER:
			water += 1
	assert_int(water).is_equal(4000 - Bucket.capacity())
	# the water channel moves the bucket into the tank beside it (ADR 0122)
	for i in 40:
		store.tick()
	assert_int(tank.buckets).is_equal(1)
	assert_int(pump.buckets).is_equal(0)
	# a puddle smaller than a bucket is left alone
	world = {}
	_fill(Vector3i(0, 19, 0), Vector3i(4, 1, 4), GameMaterials.WATER)
	assert_bool(pump.draw_from(_gv, _sv, below)).is_false()
	assert_int(pump.buckets).is_equal(0)


func test_pump_needs_power_and_the_simulation_while_it_has_room() -> void:
	var pump: PumpEntity = store.create(Vector3i(0, 20, 0), "pump")
	assert_bool(pump.needs_sim()).is_false()
	pump.load.wants = true
	pump.net_in("energy", {"volts": 48.0, "amps": 4.0, "watts": 200.0, "may_start": true})
	assert_bool(pump.needs_sim()).is_true()
	pump.buckets = Settings.pump_buckets
	assert_bool(pump.needs_sim()).is_false()
	assert_str(pump.net("energy")["kind"]).is_equal("conductor")   # full: no load, its terminals still pass the line
	assert_str(pump.use({"id": "bucket", "count": 1})["give"][0]["id"]).is_equal("water_bucket")


func _hull(at: Vector3i, dir: Vector3i, hole := true) -> void:
	var side := Vector3i(dir.z, 0, -dir.x)
	for k in range(1, 4):
		for j in range(-1, 2):
			for i in range(-1, 2):
				if hole and k == 2 and i == 0 and j == 0:
					continue
				_fill(at + dir * (k * B) + side * (i * B) + Vector3i(0, j * B, 0), Vector3i(B, B, B), GameMaterials.BRICKS)


func test_coke_oven_forms_on_a_hollow_brick_cube_and_cokes_coal() -> void:
	var oven: CokeOvenEntity = store.create(Vector3i(40, 20, 40), "coke_oven")
	assert_bool(oven.formed()).is_false()
	assert_dict(CokeOvenEntity.formed_at(_gv, oven.pos)).is_empty()
	_hull(oven.pos, Vector3i(0, 0, 1), false)   # a solid cube: not an oven
	assert_dict(CokeOvenEntity.formed_at(_gv, oven.pos)).is_empty()
	world = {}
	_hull(oven.pos, Vector3i(0, 0, 1))
	var f := CokeOvenEntity.formed_at(_gv, oven.pos)
	assert_int(f["dir"]).is_equal(2)
	assert_int(f["parts"].size()).is_equal(26)
	assert_bool(oven.set_formed(f)).is_true()
	store.refit(oven)
	assert_object(store.at(oven.pos + Vector3i(0, 0, 3 * B))).is_same(oven)   # the hull's far wall is the oven
	# no cables: it works formed
	assert_int(oven.use({"id": "coal", "count": 2})["consume"]).is_equal(2)
	assert_bool(oven.is_lit()).is_true()
	for i in 30 * 20 + 5:
		store.tick()
	assert_str(oven.output["id"]).is_equal("coal_coke")
	# a brick knocked out breaks it
	world.erase(oven.pos + Vector3i(0, 0, 3 * B))
	assert_bool(oven.set_formed(CokeOvenEntity.formed_at(_gv, oven.pos))).is_true()
	assert_bool(oven.formed()).is_false()
	assert_bool(oven.is_lit()).is_false()


func test_saves_keep_the_pump_and_the_oven() -> void:
	var pump: PumpEntity = store.create(Vector3i(0, 20, 0), "pump")
	pump.buckets = 3
	var oven: CokeOvenEntity = store.create(Vector3i(40, 20, 40), "coke_oven")
	oven.use({"id": "coal", "count": 5})
	var d := store.to_dict()
	var store2 := BlockEntities.new(items, recipes)
	assert_bool(store2.load_dict(d)).is_true()
	assert_int(store2.at(Vector3i(0, 20, 0)).buckets).is_equal(3)
	assert_str(store2.at(Vector3i(40, 20, 40)).inv.get_slot(0)["id"]).is_equal("coal")
