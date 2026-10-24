extends GdUnitTestSuite

## Hunger, body temperature and food spoilage (ADR 0040): the rules, freshness through inventory
## moves, and the native room probe.

var items: ItemRegistry


func before() -> void:
	items = ItemRegistry.load_default()


func _env(climate: float, doy: int, time: float, below := 0.0, water := false, room := false, heat := 0.0) -> float:
	return Survival.environment_c(climate, doy, 108, 9, time, below, water, room, heat)


func test_the_air_follows_season_hour_shelter_and_heat() -> void:
	var noon_summer := _env(0.0, 58, 0.58)     # mid-July, 2 pm
	var night_winter := _env(0.0, 4, 0.08)     # mid-January, 2 am
	assert_float(noon_summer).is_greater(night_winter + 15.0)
	assert_float(_env(-1.0, 58, 0.5)).is_less(_env(1.0, 58, 0.5))
	# a room pulls halfway to 18 C; a cave sits near 10 C; water is colder; heat warms more indoors
	var cold := _env(-0.8, 4, 0.08)
	assert_float(_env(-0.8, 4, 0.08, 0.0, false, true)).is_equal_approx((cold + Settings.room_c) * 0.5, 0.01)
	assert_float(_env(-0.8, 4, 0.08, 12.0)).is_equal_approx(Settings.cave_c, 0.01)
	assert_float(_env(0.0, 30, 0.5, 0.0, true)).is_less(_env(0.0, 30, 0.5))
	assert_float(_env(0.0, 30, 0.5, 0.0, false, true, 10.0) - _env(0.0, 30, 0.5, 0.0, false, true)).is_equal_approx(15.0, 0.01)


func test_the_body_eases_toward_the_air() -> void:
	assert_float(Survival.body_target(20.0)).is_equal(37.0)
	assert_float(Survival.body_target(-10.0)).is_less(33.0)   # freezing territory
	assert_float(Survival.body_target(45.0)).is_greater(39.5)
	var b := 37.0
	for i in 600:   # ten minutes at -10 C
		b = Survival.body_step(b, -10.0, 1.0, false)
	assert_float(b).is_less(Settings.freeze_c)
	var w := 37.0
	for i in 60:   # a minute in water cools faster than a minute in the same air
		w = Survival.body_step(w, -10.0, 1.0, true)
	assert_float(w).is_less(Survival.body_step(37.0, -10.0, 60.0, false))


func test_hunger_and_spoil_rates() -> void:
	var rest := Survival.hunger_rate(false, false)
	assert_float(Survival.hunger_rate(true, false)).is_equal_approx(rest * Settings.hunger_fast_mult, 1e-6)
	assert_float(Survival.hunger_rate(false, true)).is_equal_approx(rest * Settings.hunger_cold_mult, 1e-6)
	assert_float(Survival.spoil_rate(16.0)).is_equal(1.0)
	assert_float(Survival.spoil_rate(32.0)).is_equal(2.0)
	assert_float(Survival.spoil_rate(-5.0)).is_equal(0.1)
	assert_str(Survival.freshness_name(0.1)).is_equal("Fresh")
	assert_str(Survival.freshness_name(0.6)).is_equal("Stale")


func test_food_ages_into_rot() -> void:
	var inv := Inventory.new(items, 4)
	inv.add("raw_meat", 8)   # spoil_days 2
	inv.add("cobblestone", 64)
	assert_float(float(inv.get_slot(0)["spoil"])).is_equal(0.0)
	assert_bool(inv.get_slot(1).has("spoil")).is_false()
	assert_int(Survival.age_inventory(inv, 1.0, 1.0)).is_equal(0)
	assert_float(float(inv.get_slot(0)["spoil"])).is_equal_approx(0.5, 1e-6)
	assert_int(Survival.age_inventory(inv, 1.0, 1.0)).is_equal(1)
	assert_that(inv.get_slot(0)).is_equal({"id": "rot", "count": 8})
	assert_int(int(inv.get_slot(1)["count"])).is_equal(64)


func test_freshness_survives_moves_and_averages_on_merge() -> void:
	var a := Inventory.new(items, 4)
	var b := Inventory.new(items, 4)
	a.set_slot(0, {"id": "raw_meat", "count": 8, "spoil": 0.5})
	a.add("raw_meat", 8)   # fresh ones merge in: the mean
	assert_float(float(a.get_slot(0)["spoil"])).is_equal_approx(0.25, 1e-6)
	var half := a.take(0, 4)
	assert_float(float(half["spoil"])).is_equal_approx(0.25, 1e-6)
	b.put(0, half)
	assert_float(float(b.get_slot(0)["spoil"])).is_equal_approx(0.25, 1e-6)
	b.put(0, {"id": "raw_meat", "count": 4, "spoil": 0.75})
	assert_float(float(b.get_slot(0)["spoil"])).is_equal_approx(0.5, 1e-6)
	assert_bool(b.shift_click(0, a)).is_true()
	assert_float(float(a.get_slot(0)["spoil"])).is_equal_approx((12 * 0.25 + 8 * 0.5) / 20.0, 1e-6)


func _room_world() -> SimWorld:
	var sim := SimWorld.new()
	sim.configure(Vector3i(4, 2, 4), 1, 1)   # 8 x 4 x 8 m
	sim.set_materials(GameMaterials.defs())
	sim.mark_all_loaded(true)
	sim.fill_box(Vector3i(0, 0, 0), Vector3i(64, 4, 64), GameMaterials.STONE)        # the ground
	sim.fill_box(Vector3i(20, 4, 20), Vector3i(44, 24, 44), GameMaterials.STONE)     # a 3 m stone hut
	sim.fill_box(Vector3i(22, 4, 22), Vector3i(42, 22, 42), GameMaterials.AIR)       # its inside, walls 2 thick
	return sim


func _probe(sim: SimWorld, at: Vector3i) -> Dictionary:
	var open := PackedByteArray()
	open.resize(256)
	open[GameMaterials.AIR] = 1
	var heat := PackedFloat32Array()
	heat.resize(256)
	heat[GameMaterials.FIRE] = 0.4
	return ClimateProbe.new().probe(sim, at, Vector3i(64, 32, 64), 65536, open, heat, 20, 8.0)


func test_a_closed_hut_is_a_room_and_a_doorway_opens_it() -> void:
	var sim := _room_world()
	assert_bool(bool(_probe(sim, Vector3i(32, 12, 32))["enclosed"])).is_true()
	assert_bool(bool(_probe(sim, Vector3i(10, 12, 10))["enclosed"])).is_false()   # outside
	sim.fill_box(Vector3i(30, 4, 20), Vector3i(34, 14, 22), GameMaterials.AIR)     # a doorway
	assert_bool(bool(_probe(sim, Vector3i(32, 12, 32))["enclosed"])).is_false()


func test_fire_cells_give_heat_by_distance() -> void:
	var sim := _room_world()
	assert_float(float(_probe(sim, Vector3i(32, 12, 32))["heat"])).is_equal(0.0)
	sim.set_voxel(Vector3i(33, 4, 32), GameMaterials.FIRE)
	var near := float(_probe(sim, Vector3i(32, 6, 32))["heat"])
	var far := float(_probe(sim, Vector3i(32, 20, 32))["heat"])
	assert_float(near).is_greater(far)
	assert_float(far).is_greater(0.0)
