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
	var t: TurbineEntity = store.create(Vector3i(6 * B, 0, 0), "turbine")
	t.rpm = Settings.turbine_rpm
	var x := 6 * B + BlockEntities.width_of("turbine")   # the cables start at its side, whatever its size
	var cable: RefCounted = store.create(Vector3i(x, 0, 0), "cable")
	store.create(Vector3i(x + B, 0, 0), "cable")
	store.create(Vector3i(x + 2 * B, 0, 0), "electric_furnace")
	var lone: RefCounted = store.create(Vector3i(40 * B, 0, 0), "cable")
	Networks.solve(store, "energy")
	var line := Networks.look_line(cable)
	assert_str(line).contains("%d EU/t made" % roundi(Settings.turbine_eu_t)).contains("(4 parts)")
	assert_str(Networks.look_line(t)).is_equal(line)   # every part of a network reads the same
	assert_str(Networks.look_line(lone)).is_empty()     # a cable on its own is no network
	assert_str(Networks.look_line(null)).is_empty()


func test_home_and_death_waypoints_are_unique() -> void:
	var w := Waypoints.new()
	w.add("Mine", Vector3i(1, 2, 3))
	w.set_unique("home", "Home", Vector3i(10, 0, 0))
	w.set_unique("home", "Home", Vector3i(20, 0, 0))
	w.set_unique("death", "Last death", Vector3i(5, 5, 5))
	assert_int(w.list.size()).is_equal(3)
	assert_object(w.of_kind("home")["pos"]).is_equal(Vector3i(20, 0, 0))
	assert_object(w.of_kind("home")["colour"]).is_equal(Waypoints.KIND_COLOURS["home"])
	var back := Waypoints.new()
	back.load_array(w.to_array())   # saved with the player
	assert_str(String(back.of_kind("death").get("name", ""))).is_equal("Last death")
