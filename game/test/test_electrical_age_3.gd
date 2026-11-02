extends GdUnitTestSuite

## The electrical age, part 3 (ADR 0117): belts and chutes end in machines (the one transport rule:
## Transport knows every machine, so a belt's head drops into whatever it faces), a wrench turns a machine
## and the turn survives a save.

const B := Settings.BLOCK

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	store = BlockEntities.new(items, recipes)


func test_a_belt_ends_in_a_machine() -> void:
	var belt: BeltEntity = store.create(Vector3i(0, 0, 0), "belt")
	belt.facing = 1   # +x: toward the lathe
	var lathe: MachineEntity = store.create(Vector3i(B, 0, 0), "lathe")
	assert_bool(belt.accept({"id": "iron_ingot", "count": 1})).is_true()
	for i in 200:
		store.tick()
	assert_str(lathe.inv.get_slot(0).get("id", "")).is_equal("iron_ingot")
	assert_int(belt.carried.size()).is_equal(0)
	# the belt will not push what the machine refuses
	assert_bool(belt.accept({"id": "oak_log", "count": 1})).is_true()
	for i in 200:
		store.tick()
	assert_int(belt.carried.size()).is_equal(1)


func test_a_chute_feeds_a_machine_from_behind_and_a_hopper_takes_its_output() -> void:
	var chest: ChestEntity = store.create(Vector3i(-2 * B, 0, 0), "chest")   # a 1 m chest: -2B..0
	chest.inv.set_slot(0, {"id": "copper_ingot", "count": 4})
	var chute: HopperEntity = store.create(Vector3i.ZERO, "hopper")   # a chute pulls from behind (its back at -x) and drops down
	chute.item = "chute"
	chute.facing = 1
	var mill: MachineEntity = store.create(Vector3i(0, -2 * B, 0), "wiremill")
	for i in 200:
		store.tick()
	assert_str(String(mill.inv.get_slot(0).get("id", ""))).override_failure_message("mill holds %s" % [mill.inv.to_array()]).is_equal("copper_ingot")
	mill.output = {"id": "copper_wire", "count": 8}
	var hop: HopperEntity = store.create(Vector3i(0, -3 * B, 0), "hopper")   # right under the mill (its bottom at -2B): pulls its output
	for i in 200:
		store.tick()
	assert_int(int(mill.output.get("count", 0))).is_less(8)
	assert_bool(hop.inv.to_array().any(func(st: Dictionary) -> bool: return st.get("id", "") == "copper_wire")).is_true()


func test_wrench_turn_is_saved() -> void:
	var m: MachineEntity = store.create(Vector3i(0, 0, 0), "assembler")
	var plain: PackedInt32Array = m.view_parts()["solid"]["cells"]
	m.facing = 1
	assert_bool(m.view_parts()["solid"]["cells"] != plain).is_true()
	var f: ElectricFurnaceEntity = store.create(Vector3i(4 * B, 0, 0), "electric_furnace")
	f.facing = 3
	var store2 := BlockEntities.new(items, recipes)
	assert_bool(store2.load_dict(store.to_dict())).is_true()
	assert_int(store2.at(Vector3i(0, 0, 0)).facing).is_equal(1)
	assert_int(store2.at(Vector3i(4 * B, 0, 0)).facing).is_equal(3)
