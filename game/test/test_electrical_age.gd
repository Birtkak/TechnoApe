extends GdUnitTestSuite

## The electrical age, tier 1 (ADR 0113): rubber, wire, boards, circuits, parts and the first machines as
## data, and a lathe on the cables turning an ingot into rods.

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


func test_items_recipes_and_quests_exist() -> void:
	for id in ["raw_rubber", "rubber", "insulated_wire", "coated_board", "vacuum_tube", "resistor", "primitive_circuit", "iron_screw",
			"machine_hull", "electric_motor", "electric_piston", "lathe", "bending_press", "wiremill"]:
		assert_bool(items.has(id)).override_failure_message(id).is_true()
		if not id in ["rubber", "iron_screw"]:   # cured in a furnace, cut on the lathe (recipes below)
			assert_bool(recipes.recipes.has(id)).override_failure_message("recipe " + id).is_true()
	for id in ["cure_rubber", "lathe_iron_rod", "lathe_iron_screw", "press_iron_plate", "press_steel_plate", "wiremill_copper_wire"]:
		assert_bool(recipes.recipes.has(id)).override_failure_message(id).is_true()
	for m in ["lathe", "bending_press", "wiremill", "electric_furnace"]:
		assert_str(items.get_def(m).get("work", "")).is_equal(m)
		assert_str(items.get_def(m)["machine"]["station"]).is_not_empty()
	var q = JSON.parse_string(FileAccess.get_file_as_string("res://data/quests.json"))
	var ids := []
	for c in q["chapters"]:
		ids.append(c["id"])
	assert_array(ids).contains(["electrical_age"])


func test_tier_one_machines_cost_circuits_and_hulls() -> void:
	for m in ["lathe", "bending_press", "wiremill", "electric_furnace"]:
		var needs: Array = recipes.ingredient_specs(recipes.recipes[m])
		assert_bool("primitive_circuit" in needs).override_failure_message("%s needs no circuit" % m).is_true()
		assert_bool("machine_hull" in needs).override_failure_message("%s needs no hull" % m).is_true()
	# the machine's station recipes are what the recipe browser lists under it
	assert_dict(recipes.smelting_for("iron_ingot", "lathe")["output"]).contains_key_value("id", "iron_rod")
	assert_int(int(recipes.smelting_for("copper_ingot", "wiremill")["output"]["count"])).is_equal(8)
	assert_dict(recipes.smelting_for("iron_ingot", "furnace")).is_empty()   # the furnace still smelts no ingots into rods


func test_lathe_turns_rods_on_power() -> void:
	var b: BoilerEntity = store.create(Vector3i.ZERO, "boiler")
	b.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	for i in 4:
		b.use({"id": "water_bucket", "count": 1}, items.get_def("water_bucket"))
	assert_bool(b.ignite()).is_true()
	store.create(Vector3i(5 * B, 0, 0), "pipe")
	store.create(Vector3i(6 * B, 0, 0), "turbine")
	store.create(Vector3i(8 * B, 0, 0), "cable")
	var lathe: MachineEntity = store.create(Vector3i(9 * B, 0, 0), "lathe")
	assert_str(lathe.station).is_equal("lathe")
	assert_float(lathe.eu_t).is_equal(8.0)
	assert_int(lathe.use({"id": "copper_ingot", "count": 1})["consume"]).is_equal(0)   # not a lathe input
	assert_int(lathe.use({"id": "iron_ingot", "count": 1})["consume"]).is_equal(1)
	for i in 800:
		Inventory.clock_s += BlockEntities.TICK_S
		if b.pressure > 1.0:
			b.pressure = 1.0
		store.tick()
	# by now the rods are done and the lathe asks for nothing (it only draws while it has work)
	var out: Array = lathe.use({})["give"]
	assert_int(out.size()).is_equal(1)
	assert_str(out[0]["id"]).is_equal("iron_rod")
	assert_int(out[0]["count"]).is_equal(2)
