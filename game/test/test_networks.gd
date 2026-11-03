extends GdUnitTestSuite

## Machines and networks (ADR 0113): the native solver's components and shares, and the proof end to
## end: boiler -> pipe -> turbine -> cable -> electric furnace smelts without fuel.

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


func _box(x: int, w: int = B) -> PackedInt32Array:
	return PackedInt32Array([x, 0, 0, w, B, w])


# ---------- the solver ----------

func test_solver_shares_by_component_and_weakest_cable() -> void:
	var s := NetworkSolver.new()
	# source(100) - cable(32) - cable(8) - sink(50) | a lone sink(10) elsewhere
	var boxes := _box(0) + _box(B) + _box(2 * B) + _box(3 * B) + _box(10 * B)
	var roles := PackedInt32Array([NetworkSolver.SOURCE, NetworkSolver.CONDUCTOR, NetworkSolver.CONDUCTOR, NetworkSolver.SINK, NetworkSolver.SINK])
	var r := s.solve(boxes, roles, PackedFloat32Array([100, 32, 8, 50, 10]))
	assert_float(r[1]).is_equal(8.0)      # the source gives what the weakest cable carries
	assert_float(r[7]).is_equal(8.0)      # the sink gets it
	assert_float(r[9]).is_equal(0.0)      # nothing reaches the lone sink
	assert_float(r[0]).is_equal(r[6])     # one component
	assert_float(r[8]).is_not_equal(r[0])
	assert_int(s.stats()["count"]).is_equal(2)


func test_solver_shares_sources_and_fills_sinks_in_order() -> void:
	var s := NetworkSolver.new()
	# two sources (30, 10) feed two sinks (25, 20) over a wide cable: flow 40; the first sink is filled
	# whole, the second gets what is left (a machine runs whole or waits: no brownouts, ADR 0117)
	var boxes := _box(0) + _box(B) + _box(2 * B) + _box(3 * B) + _box(4 * B)
	var roles := PackedInt32Array([NetworkSolver.SOURCE, NetworkSolver.SINK, NetworkSolver.CONDUCTOR, NetworkSolver.SINK, NetworkSolver.SOURCE])
	var r := s.solve(boxes, roles, PackedFloat32Array([30, 25, 1000, 20, 10]))
	assert_float(r[1]).is_equal_approx(30.0, 0.01)
	assert_float(r[9]).is_equal_approx(10.0, 0.01)
	assert_float(r[3]).is_equal_approx(25.0, 0.01)
	assert_float(r[7]).is_equal_approx(15.0, 0.01)


func test_big_boxes_touch_by_their_faces() -> void:
	var s := NetworkSolver.new()
	# a 3-block machine at 0..3B touches a cable at 3B, and a cable above it at y = B; a diagonal one does not
	var boxes := PackedInt32Array([0, 0, 0, 3 * B, B, 3 * B]) + _box(3 * B) + PackedInt32Array([B, B, B, B, B, B]) + PackedInt32Array([3 * B, B, 3 * B, B, B, B])
	var roles := PackedInt32Array([NetworkSolver.SOURCE, NetworkSolver.SINK, NetworkSolver.SINK, NetworkSolver.SINK])
	var r := s.solve(boxes, roles, PackedFloat32Array([10, 5, 5, 5]))
	assert_float(r[3]).is_equal(5.0)
	assert_float(r[5]).is_equal(5.0)
	assert_float(r[7]).is_equal(0.0)


# ---------- the data ----------

func test_items_and_recipes_exist() -> void:
	for id in ["copper_cable", "electric_furnace"]:
		assert_bool(items.has(id)).override_failure_message(id).is_true()
		assert_bool(recipes.recipes.has(id)).override_failure_message(id).is_true()
	assert_str(items.get_def("copper_cable").get("work", "")).is_equal("cable")
	assert_str(items.get_def("electric_furnace").get("work", "")).is_equal("electric_furnace")


# ---------- end to end ----------

## A lit boiler at the origin (5 blocks wide, ADR 0099's layout), a pipe at 5B, the turbine at 6B.
func _powered_turbine() -> TurbineEntity:
	var b: BoilerEntity = store.create(Vector3i.ZERO, "boiler")
	b.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	for i in 4:
		b.use({"id": "water_bucket", "count": 1}, items.get_def("water_bucket"))
	assert_bool(b.ignite()).is_true()
	store.create(Vector3i(5 * B, 0, 0), "pipe")
	return store.create(Vector3i(6 * B, 0, 0), "turbine")


func _ticks(n: int) -> void:
	for i in n:
		Inventory.clock_s += BlockEntities.TICK_S
		var b: BoilerEntity = store.at(Vector3i.ZERO)
		if b != null and b.pressure > 1.0:
			b.pressure = 1.0
		store.tick()


func test_turbine_powers_the_furnace_over_a_cable() -> void:
	var t := _powered_turbine()
	# the turbine is 3 blocks wide: cables at 9B and 10B, the furnace at 11B
	store.create(Vector3i(9 * B, 0, 0), "cable")
	store.create(Vector3i(10 * B, 0, 0), "cable")
	var f: ElectricFurnaceEntity = store.create(Vector3i(11 * B, 0, 0), "electric_furnace")
	var r := f.use({"id": "raw_meat", "count": items.units("raw_meat")})
	assert_int(r["consume"]).is_equal(items.units("raw_meat"))
	_ticks(800)   # 40 s: the boiler reaches working pressure with the turbine drawing (ADR 0123: the dynamo must near full speed before a load joins)
	assert_bool(t.rpm > 0.0).is_true()
	assert_bool(f.is_lit() or f.smelted > 0).override_failure_message(f.load.status()).is_true()   # lit, or already done with its one piece
	assert_float(f.load.volts).is_between(40.0, 52.0)
	_ticks(260)
	assert_int(f.smelted).is_greater(0)
	assert_str(f.use({})["give"][0]["id"]).is_equal("cooked_meat")
	assert_int(Networks.networks["energy"].size()).is_greater_equal(3)   # the turbine and cables stay nodes; a finished furnace draws nothing and leaves the circuit (ADR 0123)


func test_no_cable_or_cold_boiler_means_no_power() -> void:
	_powered_turbine()
	var f: ElectricFurnaceEntity = store.create(Vector3i(12 * B, 0, 0), "electric_furnace")   # a gap: no cable
	f.use({"id": "raw_meat", "count": items.units("raw_meat")})
	_ticks(800)
	assert_bool(f.is_lit()).is_false()
	store.create(Vector3i(9 * B, 0, 0), "cable")
	store.create(Vector3i(10 * B, 0, 0), "cable")
	store.create(Vector3i(11 * B, 0, 0), "cable")
	_ticks(60)   # one solve to read the voltage, one to join
	assert_bool(f.is_lit()).is_true()
	var b: BoilerEntity = store.at(Vector3i.ZERO)
	b.pressure = 0.0
	b.lit_time = 0
	b.water = 0.0
	_ticks(60)
	assert_bool(f.is_lit()).override_failure_message("still lit: " + f.load.status()).is_false()


func test_save_keeps_the_furnace_and_the_cable() -> void:
	store.create(Vector3i(8 * B, 0, 0), "cable").axis = 1
	var f: ElectricFurnaceEntity = store.create(Vector3i(10 * B, 0, 0), "electric_furnace")
	f.use({"id": "raw_meat", "count": items.units("raw_meat")})
	var d := store.to_dict()
	var store2 := BlockEntities.new(items, recipes)
	assert_bool(store2.load_dict(d)).is_true()
	assert_int(store2.at(Vector3i(8 * B, 0, 0)).axis).is_equal(1)
	assert_str(store2.at(Vector3i(10 * B, 0, 0)).inv.get_slot(0)["id"]).is_equal("raw_meat")
