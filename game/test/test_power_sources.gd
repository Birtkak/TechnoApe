extends GdUnitTestSuite

## Round 2 of ADR 0123: the small sources (water wheel, wind turbine, hand crank), cable gauges, the lead-acid
## cell, the daylight switch and the meter panel: data, the EMF each gives at its drive, what a wheel can and
## cannot run, a thin cable burning where a thick one holds, the crank timing out, saves.

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


func _ticks(n: int) -> void:
	for i in n:
		Inventory.clock_s += BlockEntities.TICK_S
		store.tick()


func test_items_recipes_and_gauges() -> void:
	for id in ["thin_cable", "lead_acid_cell", "water_wheel", "wind_turbine", "hand_crank", "daylight_switch", "meter_panel"]:
		assert_bool(items.has(id)).override_failure_message(id).is_true()
		assert_bool(recipes.recipes.has(id)).override_failure_message("recipe " + id).is_true()
	var thin: Dictionary = items.get_def("thin_cable")["cable"]
	var mid: Dictionary = items.get_def("copper_cable")["cable"]
	var thick: Dictionary = items.get_def("heavy_copper_cable")["cable"]   # the content pass's 16 mm^2 trunk cable (ADR 0131)
	assert_bool(float(thin["ohms_per_block"]) > float(mid["ohms_per_block"]) and float(mid["ohms_per_block"]) > float(thick["ohms_per_block"])).is_true()
	assert_bool(int(thin["amps"]) < int(mid["amps"]) and int(mid["amps"]) < int(thick["amps"])).is_true()
	assert_bool("lead_acid_cell" in recipes.ingredient_specs(recipes.recipes["battery_box"])).is_true()


func test_sources_give_their_emf_by_drive() -> void:
	var wheel: PowerSourceEntity = store.create(Vector3i(0, 0, 0), "water_wheel")
	assert_str(wheel.net("energy")["kind"]).is_equal("conductor")   # still: it only passes the line
	wheel.drive = 1.0
	assert_float(wheel.net("energy")["volts"]).is_equal(50.0)
	var wind: PowerSourceEntity = store.create(Vector3i(4 * B, 0, 0), "wind_turbine")
	wind.drive = 0.5
	assert_float(wind.net("energy")["volts"]).is_equal(27.0)
	assert_float(wind.net("energy")["ohms"]).is_equal(2.0)
	var crank: PowerSourceEntity = store.create(Vector3i(8 * B, 0, 0), "hand_crank")
	assert_str(crank.net("energy")["kind"]).is_equal("conductor")
	assert_str(crank.use({})["msg"]).contains("crank")
	assert_str(crank.net("energy")["kind"]).is_equal("source")
	Inventory.clock_s += Settings.crank_s + 1.0
	assert_str(crank.net("energy")["kind"]).is_equal("conductor")   # it ran down


func test_a_wheel_runs_a_pump_but_not_a_furnace() -> void:
	var wheel: PowerSourceEntity = store.create(Vector3i(0, 0, 0), "water_wheel")
	wheel.drive = 1.0
	store.create(Vector3i(2 * B, 0, 0), "cable")
	var pump: PumpEntity = store.create(Vector3i(3 * B, 0, 0), "pump")
	_ticks(60)
	assert_bool(pump.powered()).override_failure_message(pump.load.status()).is_true()
	assert_float(pump.load.volts).is_between(44.0, 50.0)
	store.create(Vector3i(5 * B, 0, 0), "cable")
	var f: ElectricFurnaceEntity = store.create(Vector3i(6 * B, 0, 0), "electric_furnace")
	f.inv.set_slot(0, {"id": "raw_meat", "count": 4})
	_ticks(20 * 40)
	# 400 W pulls the wheel under 41 V: the furnace (the newest load) sheds, waits longer each time, never smelts; the pump holds
	assert_int(f.smelted).is_equal(0)
	assert_bool(pump.powered()).override_failure_message(pump.load.status()).is_true()
	assert_bool(f.load.retry_s >= 10).override_failure_message(f.load.status()).is_true()


func test_thin_cable_burns_where_thick_holds() -> void:
	var thin: CableEntity = store.create(Vector3i(0, 0, 0), "cable")
	thin.item = "thin_cable"
	var thick: CableEntity = store.create(Vector3i(4 * B, 0, 0), "cable")
	thick.item = "heavy_copper_cable"
	for i in 12:
		thin.net_in("energy", {"volts": 48.0, "amps": 20.0, "watts": 0.0})
		thick.net_in("energy", {"volts": 48.0, "amps": 60.0, "watts": 0.0})
	assert_bool(thin.burnt).is_true()
	assert_bool(thick.burnt).is_false()
	assert_float(thick.net("energy")["ohms"]).is_equal(0.00053)


func test_daylight_switch_and_meter_panel() -> void:
	var sw: CircuitEntity = store.create(Vector3i(0, 0, 0), "daylight_switch")
	assert_bool(sw.open).is_false()
	sw.open = true   # WorkSystem sets it by the hour
	assert_dict(sw.net("energy")).is_empty()
	assert_str(sw.look_lines()[0]).contains("daylight")
	sw.net_in("energy", {"volts": 48.0, "amps": 50.0, "watts": 0.0})   # it never trips
	sw.open = false
	assert_str(sw.net("energy")["kind"]).is_equal("conductor")
	var panel: MeterPanelEntity = store.create(Vector3i(4 * B, 0, 0), "meter_panel")
	panel.net_in("energy", {"volts": 47.3, "amps": 12.0, "watts": 0.0})
	assert_str(panel.look_lines()[0]).contains("47.3 V, 12.0 A")
	var cells: PackedInt32Array = panel.view_parts()["solid"]["cells"]
	panel.net_in("energy", {"volts": 0.0, "amps": 0.0, "watts": 0.0})
	assert_bool(panel.view_parts()["solid"]["cells"] != cells).is_true()   # the bars fell


func test_sources_in_parallel_add_up_and_save() -> void:
	var wheel: PowerSourceEntity = store.create(Vector3i(0, 0, 0), "water_wheel")
	wheel.drive = 1.0
	store.create(Vector3i(2 * B, 0, 0), "cable")
	var crank: PowerSourceEntity = store.create(Vector3i(3 * B, 0, 0), "hand_crank")
	crank.cranked_until = Inventory.clock_s + 1000.0
	store.create(Vector3i(4 * B, 0, 0), "cable")
	var lamp: ElectricLampEntity = store.create(Vector3i(5 * B, 0, 0), "electric_lamp")
	_ticks(60)
	assert_bool(lamp.is_lit()).is_true()
	assert_bool(wheel.amps > 0.0 and crank.amps > 0.0).override_failure_message("wheel %.2f A, crank %.2f A" % [wheel.amps, crank.amps]).is_true()
	var store2 := BlockEntities.new(items, recipes)
	assert_bool(store2.load_dict(store.to_dict())).is_true()
	assert_str(store2.at(Vector3i(0, 0, 0)).kind).is_equal("water_wheel")
	assert_str(store2.at(Vector3i(3 * B, 0, 0)).kind).is_equal("hand_crank")
