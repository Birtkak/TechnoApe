extends GdUnitTestSuite

## The electrical age (ADR 0113, 0117): rubber, wire, boards, circuits, parts and the LV machines as data;
## machines on the cables doing their work (lathe, macerator, two-input alloy smelter, assembler), the
## battery box charging from what the machines leave and giving when the boiler goes cold, the lamp.

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


## A lit boiler at the origin, a pipe at 5B, the turbine at 6B (3 blocks wide), a cable at 9B: machines go at 10B.
func _powered_line() -> BoilerEntity:
	var b: BoilerEntity = store.create(Vector3i.ZERO, "boiler")
	b.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	for i in 4:
		b.use({"id": "water_bucket", "count": 1}, items.get_def("water_bucket"))
	assert_bool(b.ignite()).is_true()
	store.create(Vector3i(5 * B, 0, 0), "pipe")
	store.create(Vector3i(6 * B, 0, 0), "turbine")
	store.create(Vector3i(9 * B, 0, 0), "cable")
	return b


func _ticks(n: int, b: BoilerEntity) -> void:
	for i in n:
		Inventory.clock_s += BlockEntities.TICK_S
		if b.pressure > 1.0:
			b.pressure = 1.0
		store.tick()


func test_items_recipes_and_quests_exist() -> void:
	for id in ["raw_rubber", "rubber", "insulated_wire", "coated_board", "vacuum_tube", "resistor", "primitive_circuit", "iron_screw",
			"machine_hull", "electric_motor", "electric_piston", "lathe", "bending_press", "wiremill", "macerator", "alloy_smelter",
			"assembler", "battery_box", "electric_lamp"]:
		assert_bool(items.has(id)).override_failure_message(id).is_true()
		if not id in ["rubber", "iron_screw"]:   # cured in a furnace, cut on the lathe (recipes below)
			assert_bool(recipes.recipes.has(id)).override_failure_message("recipe " + id).is_true()
	for id in ["cure_rubber", "lathe_iron_rod", "lathe_iron_screw", "press_iron_plate", "press_steel_plate", "wiremill_copper_wire",
			"macerate_iron_ore", "alloy_bronze", "alloy_steel", "assemble_primitive_circuit", "assemble_electric_motor"]:
		assert_bool(recipes.recipes.has(id)).override_failure_message(id).is_true()
	for m in ["lathe", "bending_press", "wiremill", "macerator", "alloy_smelter", "assembler", "electric_furnace"]:
		assert_str(items.get_def(m).get("work", "")).is_equal(m)
		assert_bool(items.get_def(m).has("machine")).override_failure_message(m).is_true()
	var q = JSON.parse_string(FileAccess.get_file_as_string("res://data/quests.json"))
	var ids := []
	for c in q["chapters"]:
		ids.append(c["id"])
	assert_array(ids).contains(["electrical_age"])


func test_tier_one_machines_cost_circuits_and_hulls() -> void:
	for m in ["lathe", "bending_press", "wiremill", "macerator", "alloy_smelter", "assembler", "electric_furnace"]:
		var needs: Array = recipes.ingredient_specs(recipes.recipes[m])
		assert_bool("primitive_circuit" in needs).override_failure_message("%s needs no circuit" % m).is_true()
		assert_bool("machine_hull" in needs).override_failure_message("%s needs no hull" % m).is_true()
	# machine work is process recipes under the machine's station (ADR 0115): the browser lists them there
	assert_array(recipes.by_station("lathe")).contains(["lathe_iron_rod", "lathe_iron_screw"])
	assert_dict(recipes.smelting_for("iron_ingot", "furnace")).is_empty()   # the furnace still smelts no ingots into rods


func test_lathe_turns_rods_on_power() -> void:
	var b := _powered_line()
	var lathe: MachineEntity = store.create(Vector3i(10 * B, 0, 0), "lathe")
	assert_float(lathe.load.watts).is_equal(200.0)
	assert_int(lathe.use({"id": "copper_ingot", "count": 1})["consume"]).is_equal(0)   # not a lathe input
	assert_int(lathe.use({"id": "iron_ingot", "count": 1})["consume"]).is_equal(1)
	_ticks(800, b)   # pressure takes ~35 s with the turbine drawing; the dynamo must near full speed before a load joins (ADR 0123)
	assert_bool(lathe.is_lit()).override_failure_message(lathe.load.status()).is_true()
	_ticks(400, b)
	var out: Array = lathe.use({})["give"]
	assert_int(out.size()).is_equal(1)
	assert_str(out[0]["id"]).is_equal("iron_rod")
	assert_int(out[0]["count"]).is_equal(2)
	assert_bool(lathe.is_lit()).is_false()   # nothing left to do: it asks for no power


func test_macerator_triples_and_a_hopper_can_unload_it() -> void:
	var b := _powered_line()
	var m: MachineEntity = store.create(Vector3i(10 * B, 0, 0), "macerator")
	assert_int(m.use({"id": "iron_ore", "count": items.units("iron_ore")})["consume"]).is_equal(items.units("iron_ore"))
	_ticks(900, b)   # ~35 s to full dynamo speed, then 4 s of work
	assert_int(int(m.output.get("count", 0))).is_equal(3)
	assert_str(Transport.pull(m)["id"]).is_equal("crushed_iron_ore")
	assert_int(int(m.output["count"])).is_equal(2)
	assert_int(Transport.give(m, {"id": "copper_ore", "count": 8}, items)).is_equal(1)


func test_alloy_smelter_takes_two_inputs() -> void:
	var b := _powered_line()
	var a: MachineEntity = store.create(Vector3i(10 * B, 0, 0), "alloy_smelter")
	assert_int(a.inv.size()).is_equal(2)
	a.use({"id": "copper_ingot", "count": 3})
	assert_dict(a.current()).is_empty()   # tin missing
	a.use({"id": "tin_nugget", "count": 3})
	assert_str(a.current()["id"]).is_equal("alloy_bronze")
	_ticks(1000, b)   # ~35 s to dynamo speed, 10 s of work
	assert_str(a.use({})["give"][0]["id"]).is_equal("bronze_ingot")
	assert_bool(a.inv.is_empty_slot(0) and a.inv.is_empty_slot(1)).is_true()


func test_assembler_doubles_circuits() -> void:
	var b := _powered_line()
	var a: MachineEntity = store.create(Vector3i(10 * B, 0, 0), "assembler")
	for id in ["vacuum_tube", "resistor", "capacitor", "insulated_wire", "coated_board"]:   # a capacitor since ADR 0123
		a.use({"id": id, "count": 2})
	assert_str(a.current()["id"]).is_equal("assemble_primitive_circuit")
	_ticks(1000, b)
	var out: Dictionary = a.use({})["give"][0]
	assert_str(out["id"]).is_equal("primitive_circuit")
	assert_int(int(out["count"])).is_equal(2)


func test_low_sinks_only_get_what_sinks_leave() -> void:
	var s := NetworkSolver.new()
	# source 20 - sink 16 - low sink 32: the low sink gets the 4 left; with a 100 source it gets 32
	var boxes := PackedInt32Array([0, 0, 0, B, B, B, B, 0, 0, B, B, B, 2 * B, 0, 0, B, B, B])
	var roles := PackedInt32Array([NetworkSolver.SOURCE, NetworkSolver.SINK, NetworkSolver.SINK_LOW])
	var r := s.solve(boxes, roles, PackedFloat32Array([20, 16, 32]))
	assert_float(r[3]).is_equal(16.0)
	assert_float(r[5]).is_equal(4.0)
	assert_float(r[1]).is_equal(20.0)
	r = s.solve(boxes, roles, PackedFloat32Array([100, 16, 32]))
	assert_float(r[5]).is_equal(32.0)


func test_saves_keep_machine_state() -> void:
	var a: MachineEntity = store.create(Vector3i(10 * B, 0, 0), "alloy_smelter")
	a.use({"id": "tin_nugget", "count": 3})
	a.output = {"id": "bronze_ingot", "count": 4}
	var bat: BatteryBoxEntity = store.create(Vector3i(13 * B, 0, 0), "battery_box")
	bat.charge = 500.0
	var d := store.to_dict()
	var store2 := BlockEntities.new(items, recipes)
	assert_bool(store2.load_dict(d)).is_true()
	var a2: MachineEntity = store2.at(Vector3i(10 * B, 0, 0))
	assert_str(a2.inv.get_slot(0)["id"]).is_equal("tin_nugget")
	assert_int(int(a2.output["count"])).is_equal(4)
	assert_float(store2.at(Vector3i(13 * B, 0, 0)).charge).is_equal(500.0)
