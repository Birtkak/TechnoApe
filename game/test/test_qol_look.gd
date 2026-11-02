extends GdUnitTestSuite

## ADR 0130: the look-at panel's network line (what the energy network an entity is on makes, wants
## and moves) from the last solve.

const B := Settings.BLOCK

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	store = BlockEntities.new(items, recipes)


func test_network_line_sums_the_component() -> void:
	var t: TurbineEntity = store.create(Vector3i(6 * B, 0, 0), "turbine")   # three blocks wide (ADR 0129)
	t.rpm = Settings.turbine_rpm
	var cable: RefCounted = store.create(Vector3i(9 * B, 0, 0), "cable")
	store.create(Vector3i(10 * B, 0, 0), "cable")
	store.create(Vector3i(11 * B, 0, 0), "electric_furnace")
	var lone: RefCounted = store.create(Vector3i(40 * B, 0, 0), "cable")
	Networks.solve(store, "energy")
	var line := Networks.look_line(cable)
	assert_str(line).contains("%d EU/t made" % roundi(Settings.turbine_eu_t)).contains("(4 parts)")
	assert_str(Networks.look_line(t)).is_equal(line)   # every part of a network reads the same
	assert_str(Networks.look_line(lone)).is_empty()     # a cable on its own is no network
	assert_str(Networks.look_line(null)).is_empty()
