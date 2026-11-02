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


func test_chat_history_survives_a_restart() -> void:
	var path := "user://test_chat_history.txt"
	DirAccess.remove_absolute(ProjectSettings.globalize_path(path))
	var a: ChatBox = auto_free(ChatBox.new())
	a.history_path = path
	add_child(a)
	a._on_submitted("hello")
	a._on_submitted("/help")
	var b: ChatBox = auto_free(ChatBox.new())
	b.history_path = path
	add_child(b)
	assert_array(b._history).contains_exactly(["hello", "/help"])
	b._history_step(-1)   # Up recalls the newest
	assert_str(b._field.text).is_equal("/help")
	DirAccess.remove_absolute(ProjectSettings.globalize_path(path))


func test_light_level_falls_off_to_each_sources_range() -> void:
	var torch := [0.0, Vector3.ZERO, {}]   # a torch's light: torch_light_range_m
	var r := Settings.m2v(Settings.torch_light_range_m)
	assert_int(LightOverlay.light_level(Vector3.ZERO, [torch])).is_equal(15)
	assert_int(LightOverlay.light_level(Vector3(r * 0.5, 0, 0), [torch])).is_equal(8)   # LIT: lit up to half the range
	assert_int(LightOverlay.light_level(Vector3(r * 2.0, 0, 0), [torch])).is_equal(0)
	var fire := [0.0, Vector3(r * 2.0, 0, 0), {"range_m": Settings.torch_light_range_m * 2.0}]
	assert_int(LightOverlay.light_level(Vector3(r * 1.5, 0, 0), [torch, fire])).is_equal(11)   # the brightest wins
	assert_int(LightOverlay.light_level(Vector3.ZERO, [])).is_equal(0)
