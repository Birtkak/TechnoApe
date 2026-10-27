extends GdUnitTestSuite

## Stage 3 transport (ADR 0100): a hopper pulls from the chest above it and feeds a belt, the belt
## carries into the next belt and the hopper at its end, a chute drops down, rails join round
## corners and a cart flag rides them, saves.

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities

const B := Settings.BLOCK


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	store = BlockEntities.new(items, recipes)


func _ticks(n: int) -> void:
	for i in n:
		store.tick()


func test_items_and_recipes_exist() -> void:
	for id in ["hopper", "chute", "belt", "rail", "minecart", "chest_cart"]:
		assert_bool(items.has(id)).override_failure_message(id).is_true()
		assert_bool(recipes.recipes.has(id)).override_failure_message(id).is_true()
	assert_int(Transport.facing_from_yaw(0.0)).is_equal(0)
	assert_bool(Transport.dir_of(0) == Vector3i(0, 0, -B)).is_true()
	assert_bool(Transport.dir_of(1) == Vector3i(B, 0, 0)).is_true()


func test_hopper_pulls_from_a_chest_and_belts_carry_to_the_next_hopper() -> void:
	var chest: ChestEntity = store.create(Vector3i(0, B, 0), "chest")
	chest.inv.add("coal", 3)
	var h: HopperEntity = store.create(Vector3i.ZERO, "hopper")   # faces -z
	var b1: BeltEntity = store.create(Vector3i(0, 0, -B), "belt")
	var b2: BeltEntity = store.create(Vector3i(0, 0, -2 * B), "belt")
	var h2: HopperEntity = store.create(Vector3i(0, 0, -3 * B), "hopper")
	_ticks(int((Settings.hopper_s * 3 + Settings.belt_s * 2 + 2) * 20))
	assert_bool(chest.inv.is_empty_slot(0)).is_true()
	var got := 0
	for s in h2.inv.to_array():
		if not s.is_empty() and s["id"] == "coal":
			got += int(s["count"])
	assert_int(got + b1.carried.size() + b2.carried.size()).is_equal(3)
	assert_int(got).is_greater(0)
	assert_int(h.moved).is_greater(0)
	# an empty hand empties a hopper
	var res := h2.use({})
	assert_int(res["give"].size()).is_greater(0)


func test_chute_drops_into_the_piece_below_and_a_belt_hands_to_a_machine() -> void:
	var c: HopperEntity = store.create(Vector3i(0, B, 0), "hopper")
	c.item = "chute"
	c.use({"id": "gravel", "count": 1})
	var sieve: SteamSieveEntity = store.create(Vector3i.ZERO, "steam_sieve")
	_ticks(int(Settings.hopper_s * 20) + 5)
	assert_int(sieve.loads).is_equal(1)
	assert_str(sieve.input).is_equal("gravel")
	var belt: BeltEntity = store.create(Vector3i(B, 0, 0), "belt")
	belt.facing = 3   # -x: into the sieve
	assert_bool(belt.accept({"id": "sand", "count": 1})).is_true()
	assert_str(belt.use({"id": "sand", "count": 1})["msg"]).is_equal("The belt is full.")   # no gap yet
	_ticks(10)
	assert_str(belt.use({"id": "sand", "count": 1})["msg"]).is_equal("")
	_ticks(int(Settings.belt_s * 20) + 10)
	assert_int(sieve.loads).is_equal(1)   # it sifts gravel now, sand waits on the belt
	assert_int(belt.carried.size()).is_equal(2)


func test_rails_join_round_corners_and_carry_the_cart_flag() -> void:
	var r0: RailEntity = store.create(Vector3i.ZERO, "rail")
	var r1: RailEntity = store.create(Vector3i(0, 0, B), "rail")
	var r2: RailEntity = store.create(Vector3i(B, 0, B), "rail")
	r2.axis = 1
	var n := RailEntity.next_of(store, r0, Vector3i(0, 0, B))
	assert_object(n["rail"]).is_same(r1)
	n = RailEntity.next_of(store, r1, Vector3i(0, 0, B))   # nothing straight on: round the corner to +x
	assert_object(n["rail"]).is_same(r2)
	assert_bool(n["dir"] == Vector3i(B, 0, 0)).is_true()
	assert_bool(RailEntity.next_of(store, r2, Vector3i(B, 0, 0)).is_empty()).is_true()
	r2.cart = "minecart"
	assert_array(r2.contents()).contains([{"id": "minecart", "count": 1}])
	var d: Dictionary = JSON.parse_string(JSON.stringify(store.to_dict()))
	var again := BlockEntities.new(items, recipes)
	assert_bool(again.load_dict(d)).is_true()
	assert_str(again.at(Vector3i(B, 0, B)).cart).is_equal("minecart")
	assert_int(again.at(Vector3i(B, 0, B)).axis).is_equal(1)


func test_slopes_climb_a_block_and_the_wrench_cycles_them() -> void:
	var r0: RailEntity = store.create(Vector3i.ZERO, "rail")
	var s: RailEntity = store.create(Vector3i(0, 0, B), "rail")
	s.turn()   # up +z
	assert_bool(s.rise == Vector3i(0, 0, B)).is_true()
	var r2: RailEntity = store.create(Vector3i(0, B, 2 * B), "rail")
	var d := Vector3i(0, 0, B)
	assert_object(RailEntity.next_of(store, r0, d)["rail"]).is_same(s)
	assert_object(RailEntity.next_of(store, s, d)["rail"]).is_same(r2)   # up the slope
	assert_object(RailEntity.next_of(store, r2, -d)["rail"]).is_same(s)   # down onto it from above
	assert_object(RailEntity.next_of(store, s, -d)["rail"]).is_same(r0)
	assert_float(RailEntity.height_on(s, d, 0.5)).is_equal(B * 0.5)
	assert_float(RailEntity.height_on(s, -d, 0.25)).is_equal(B * 0.75)
	assert_float(RailEntity.height_on(r0, d, 0.5)).is_equal(0.0)
	for i in 5:
		s.turn()
	assert_bool(s.rise == Vector3i.ZERO and s.axis == 0).is_true()   # six states round


func test_chest_cart_rolls_to_the_end_and_hoppers_load_and_empty_it() -> void:
	var rails: Array = []
	for i in 4:
		rails.append(store.create(Vector3i(0, 0, i * B), "rail"))
	var top: HopperEntity = store.create(Vector3i(0, B, 0), "hopper")
	top.use({"id": "coal", "count": 1})
	var first: RailEntity = rails[0]
	first.cart = "chest_cart"
	_ticks(int(Settings.hopper_s * 20) + 5)
	assert_str(first.cargo.get_slot(0).get("id", "")).is_equal("coal")   # the hopper above loaded it
	first.rolling = Vector3i(0, 0, B)
	_ticks(int(3 * B / Settings.m2v(Settings.minecart_speed_m) * 20) + 20)
	var last: RailEntity = rails[3]
	assert_str(last.cart).is_equal("chest_cart")
	assert_str(first.cart).is_equal("")
	assert_bool(last.rolling == Vector3i.ZERO).is_true()
	assert_str(last.cargo.get_slot(0).get("id", "")).is_equal("coal")
	var bottom: HopperEntity = store.create(Vector3i(0, -B, 3 * B), "hopper")
	_ticks(int(Settings.hopper_s * 20) + 5)
	assert_bool(last.cargo.is_empty_slot(0)).is_true()
	assert_str(bottom.inv.get_slot(0).get("id", "")).is_equal("coal")
	var d: Dictionary = JSON.parse_string(JSON.stringify(store.to_dict()))
	var again := BlockEntities.new(items, recipes)
	assert_bool(again.load_dict(d)).is_true()
	assert_str(again.at(Vector3i(0, 0, 3 * B)).cart).is_equal("chest_cart")


func test_saves_keep_hopper_and_belt_loads() -> void:
	var h: HopperEntity = store.create(Vector3i.ZERO, "hopper")
	h.facing = 2
	h.use({"id": "coal", "count": 1})
	var b: BeltEntity = store.create(Vector3i(B, 0, 0), "belt")
	b.accept({"id": "sand", "count": 1})
	var d: Dictionary = JSON.parse_string(JSON.stringify(store.to_dict()))
	var again := BlockEntities.new(items, recipes)
	assert_bool(again.load_dict(d)).is_true()
	assert_int(again.at(Vector3i.ZERO).facing).is_equal(2)
	assert_str(again.at(Vector3i.ZERO).inv.get_slot(0)["id"]).is_equal("coal")
	assert_int(again.at(Vector3i(B, 0, 0)).carried.size()).is_equal(1)
