extends GdUnitTestSuite

## ADR 0162: the smokehouse, the salt pan, salted fish, the cellar's effect on cured food, and the rating
## of the room around a bed (pure maths over a made-up voxel grid, no world).

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	store = BlockEntities.new(items, recipes)
	StoneAge.now_days = 10.0


func _hours(n: int) -> void:
	for h in n:
		StoneAge.now_days += 1.0 / 24.0
		store.tick()


func test_data_loads_clean() -> void:
	assert_array(items.errors).is_empty()
	assert_array(recipes.errors).is_empty()
	for id in ["smokehouse", "salt_pan", "salted_fish"]:
		assert_bool(items.has(id)).is_true()


func test_smokehouse_smokes_over_time_and_faster_than_a_rack() -> void:
	var house := store.create(Vector3i(0, 0, 0), "smokehouse") as RackEntity
	var rack := store.create(Vector3i(8, 0, 0), "rack") as RackEntity
	house.smoky = true
	rack.smoky = true
	for i in 12:
		assert_int(house.use(items.get_def("raw_meat"))["consume"]).is_equal(1)
	assert_int(house.use(items.get_def("raw_meat"))["consume"]).is_equal(0)   # twelve hooks
	rack.use(items.get_def("raw_meat"))
	store.tick()
	_hours(5)
	assert_str(String(house.hung[0]["done"])).is_equal("")   # 5 h of 10 h / 1.5: not yet
	_hours(2)
	assert_str(String(house.hung[0]["done"])).is_equal("smoked_meat")   # 6.67 h
	assert_str(String(rack.hung[0]["done"])).is_equal("")
	_hours(4)
	assert_str(String(rack.hung[0]["done"])).is_equal("smoked_meat")


func test_salt_pan_evaporates_only_in_the_sun() -> void:
	var pan := store.create(Vector3i(0, 0, 0), "salt_pan") as RackEntity
	assert_int(pan.use(items.get_def("brine_pot"))["consume"]).is_equal(1)
	assert_int(pan.use(items.get_def("raw_meat"))["consume"]).is_equal(0)
	store.tick()
	_hours(20)   # night, under a roof: nothing
	assert_float(float(pan.hung[0]["p"])).is_equal(0.0)
	pan.smoky = true
	_hours(8)
	assert_array(pan.use({})["give"]).is_equal([{"id": "salt_pot", "count": 1}])


func test_salted_fish_keeps_longer_than_raw_and_smoked_in_a_cellar() -> void:
	var fresh := {"id": "raw_fish", "count": 1, "spoil": 0.0}
	var salted := {"id": "salted_fish", "count": 1, "spoil": 0.0}
	var inv := Inventory.new(items, 2)
	inv.set_slot(0, fresh)
	inv.set_slot(1, salted)
	Survival.age_inventory(inv, 0.5, 1.0)
	assert_float(float(inv.get_slot(1)["spoil"])).is_less(float(inv.get_slot(0).get("spoil", 1.0)) * 0.1)
	var summer := 28.0
	assert_float(Seasons.container_rate(5.0, true, summer, "chest")).is_less(Seasons.container_rate(0.0, false, summer, "chest") * 0.3)


func _room(closed: bool) -> Callable:
	# a 10 x 10 voxel room around the origin, floor at y = -2: walls at |x|,|z| = 6 and a roof at y = 20
	return func(p: Vector3i) -> bool:
		if p.y <= -2:
			return true
		if not closed:
			return false
		return absi(p.x) >= 6 or absi(p.z) >= 6 or p.y >= 20


func test_closed_lit_warm_room_rates_higher_than_open_field() -> void:
	var from := Vector3i(0, 2, 0)
	var closed := Comfort.enclosure(_room(true), from)
	var open := Comfort.enclosure(_room(false), from)
	assert_float(closed).is_equal_approx(1.0, 0.001)
	assert_float(open).is_equal(0.0)
	var cosy := Comfort.score(closed, true, 5.0, 4)
	var field := Comfort.score(open, false, 0.0, 0)
	assert_int(cosy).is_equal(100)
	assert_int(field).is_equal(0)
	assert_str(Comfort.tier(cosy)).is_equal("Luxurious")
	assert_str(Comfort.tier(field)).is_equal("Bare")
	assert_dict(Comfort.buff(field)).is_empty()
	var b := Comfort.buff(cosy)
	assert_str(String(b["kind"])).is_equal("rested")
	assert_float(float(Comfort.buff(cosy, true)["s"])).is_equal_approx(float(b["s"]) * 0.5, 0.01)
	# a roof and walls alone (unlit, cold, bare) are not enough to rest
	assert_dict(Comfort.buff(Comfort.score(closed, false, 0.0, 0))).is_empty()
