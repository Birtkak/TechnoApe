extends GdUnitTestSuite

## Steam and water as network channels (ADR 0122): a boiler feeds a turbine over a long pipe run, the
## weakest pipe caps a run, a pump fills a tank over pipes and the tank keeps a boiler wet, saves.

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


func _boiler(at := Vector3i.ZERO) -> BoilerEntity:
	var b: BoilerEntity = store.create(at, "boiler")
	b.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	for i in 4:
		b.use({"id": "water_bucket", "count": 1}, items.get_def("water_bucket"))
	assert_bool(b.ignite()).is_true()
	return b


func _ticks(n: int, b: BoilerEntity = null) -> void:
	for i in n:
		Inventory.clock_s += BlockEntities.TICK_S
		if b != null and b.pressure > 1.0:
			b.pressure = 1.0
		store.tick()


func test_steam_reaches_a_turbine_over_thirty_pipes() -> void:
	var b := _boiler()
	for i in range(5, 35):
		store.create(Vector3i(i * B, 0, 0), "pipe")
	var t: TurbineEntity = store.create(Vector3i(35 * B, 0, 0), "turbine")
	_ticks(20 * 25, b)
	assert_object(t.boiler).is_same(b)
	assert_float(t.rpm).is_greater(0.0)
	var net: Dictionary = Networks.networks["steam"][t.get_instance_id()]
	assert_int(net["nodes"]).is_equal(32)
	assert_float(net["cap"]).is_equal(Settings.pipe_steam_cap)
	# a pipe pulled out: the turbine stops next solve
	store.remove(Vector3i(20 * B, 0, 0))
	_ticks(40, b)
	assert_object(t.boiler).is_null()
	assert_float(t.rpm).is_equal(0.0)


func test_the_weakest_pipe_caps_a_run_and_machines_take_turns() -> void:
	# a solver-level check with pipe numbers: a bronze main with one copper pipe carries 0.5 bar/s;
	# sinks of 0.3 each: the first two run, the third waits (sinks fill in order, ADR 0117)
	var s := NetworkSolver.new()
	var boxes := PackedInt32Array()
	var roles := PackedInt32Array()
	var amounts := PackedFloat32Array()
	var plan := [[NetworkSolver.SOURCE, 2.0], [NetworkSolver.CONDUCTOR, 1.0], [NetworkSolver.CONDUCTOR, 0.5], [NetworkSolver.SINK, 0.3],
		[NetworkSolver.CONDUCTOR, 1.0], [NetworkSolver.SINK, 0.3], [NetworkSolver.SINK, 0.3]]
	for i in plan.size():
		boxes.append_array(PackedInt32Array([i * B, 0, 0, B, B, B]))
		roles.append(plan[i][0])
		amounts.append(plan[i][1])
	var r := s.solve(boxes, roles, amounts)
	assert_float(r[3 * 2 + 1]).is_equal_approx(0.3, 0.001)
	assert_float(r[5 * 2 + 1]).is_equal_approx(0.2, 0.001)
	assert_float(r[6 * 2 + 1]).is_equal(0.0)


func test_pump_fills_a_tank_over_pipes_and_the_tank_keeps_the_boiler_wet() -> void:
	var pump: PumpEntity = store.create(Vector3i(0, 0, 0), "pump")
	pump.buckets = 4
	store.create(Vector3i(2 * B, 0, 0), "pipe")
	store.create(Vector3i(3 * B, 0, 0), "pipe")
	var tank: TankEntity = store.create(Vector3i(4 * B, 0, 0), "tank")
	_ticks(20 * 4)
	assert_int(tank.buckets).is_greater_equal(3)   # pipe_water_cap a second, a second of lag
	assert_int(pump.buckets + tank.buckets).is_equal(4)
	# a dry boiler further down the pipes drinks from the tank
	store.create(Vector3i(6 * B, 0, 0), "pipe")
	var b: BoilerEntity = store.create(Vector3i(7 * B, 0, 0), "boiler")
	b.water = 0.0
	_ticks(20 * 4)
	assert_bool(b.water >= Settings.boiler_bucket_fill).override_failure_message("boiler water %.2f, tank %d, giving %s" % [b.water, tank.buckets, tank.giving]).is_true()


func test_saves_keep_the_tank_state() -> void:
	var tank: TankEntity = store.create(Vector3i(4 * B, 0, 0), "tank")
	tank.buckets = 5
	tank.giving = true
	var store2 := BlockEntities.new(items, recipes)
	assert_bool(store2.load_dict(store.to_dict())).is_true()
	assert_int(store2.at(Vector3i(4 * B, 0, 0)).buckets).is_equal(5)
	assert_bool(store2.at(Vector3i(4 * B, 0, 0)).giving).is_true()
