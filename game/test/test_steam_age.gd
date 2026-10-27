extends GdUnitTestSuite

## The steam age, stage 1 (ADR 0097): plates, rods and pipes smithed on the anvil, the small
## boiler's pressure, dry and over-pressure bursts, the steam hammer smithing by itself over a pipe
## run, the steam sieve sifting by itself, rust and the mending plate, saves.

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities

const B := Settings.BLOCK


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


func _hot(id: String) -> Dictionary:
	return {"id": id, "count": 1, "hot_until": Inventory.clock_s + Settings.metal_cool_s}


## A lit boiler with a full tank at the origin.
func _boiler() -> BoilerEntity:
	var b: BoilerEntity = store.create(Vector3i.ZERO, "boiler")
	b.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	for i in 4:
		b.use({"id": "water_bucket", "count": 1}, items.get_def("water_bucket"))
	assert_bool(b.ignite()).is_true()
	return b


# ---------- data ----------

func test_items_and_recipes_exist() -> void:
	for id in ["iron_plate", "iron_rod", "bronze_pipe", "small_boiler", "steam_hammer", "steam_sieve", "goggles", "leather_apron"]:
		assert_bool(items.has(id)).override_failure_message(id).is_true()
	assert_bool(recipes.recipes.has("smith_iron_plate")).is_true()
	assert_int(AnvilEntity.ingots_for(recipes.recipes["smith_iron_plate"])).is_equal(1)
	assert_int(AnvilEntity.ingots_for(recipes.recipes["smith_iron_rod"])).is_equal(1)
	assert_int(int(recipes.recipes["smith_iron_rod"]["output"]["count"])).is_equal(2)
	for id in ["small_boiler", "steam_hammer", "steam_sieve", "goggles", "leather_apron"]:
		assert_bool(recipes.recipes.has(id)).override_failure_message(id).is_true()
	assert_bool(items.get_def("goggles")["clothing"].get("steam_proof", false)).is_true()


# ---------- the boiler ----------

func test_boiler_takes_coal_water_and_builds_pressure() -> void:
	var b := _boiler()
	assert_float(b.water).is_equal(1.0)
	var bucket := b.use({"id": "water_bucket", "count": 1}, items.get_def("water_bucket"))
	assert_str(bucket["msg"]).is_equal("The tank is full.")
	assert_bool(b.may_place(FirepitEntity.INPUT, "iron_ingot")).is_false()
	_ticks(int(Settings.boiler_build_s * 10))
	assert_float(b.pressure).is_between(0.4, 0.6)
	assert_float(b.water).is_less(1.0)
	assert_float(b.wear).is_greater(0.0)
	# an empty hand vents half
	var vent := b.use({}, {})
	assert_str(vent["msg"]).is_equal("Steam vents with a hiss.")
	assert_float(b.pressure).is_between(0.2, 0.3)


func test_boiler_bursts_over_pressure_and_when_dry() -> void:
	var b := _boiler()
	var bursts := []
	b.burst.connect(func(p: Vector3i) -> void: bursts.append(p))
	_ticks(int(Settings.boiler_build_s * Settings.boiler_burst_at * 20) + 40)
	assert_array(bursts).is_equal([Vector3i.ZERO])
	# dry: whistle for boiler_dry_s, then the blast
	var d: BoilerEntity = store.create(Vector3i(0, 0, 32), "boiler")
	d.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	d.ignite()
	var dry := []
	d.burst.connect(func(p: Vector3i) -> void: dry.append(p))
	_ticks(int(Settings.boiler_dry_s * 20) - 5)
	assert_bool(d.screaming()).is_true()
	assert_array(dry).is_empty()
	_ticks(10)
	assert_array(dry).is_equal([Vector3i(0, 0, 32)])


func test_boiler_rust_mends_with_a_plate() -> void:
	var b := _boiler()
	b.wear = 0.9
	var res := b.use({"id": "iron_plate", "count": 1}, items.get_def("iron_plate"))
	assert_int(res["consume"]).is_equal(1)
	assert_float(b.wear).is_equal(0.0)
	assert_str(b.use({"id": "flint", "count": 1}, items.get_def("flint"))["msg"]).is_equal("The boiler takes coal, water buckets and iron plates.")


# ---------- pipes and the steam hammer ----------

func test_steam_hammer_smiths_a_plate_over_pipes() -> void:
	var b := _boiler()
	store.create(Vector3i(5 * B, 0, 0), "pipe")   # the boiler is 5 blocks wide
	store.create(Vector3i(6 * B, 0, 0), "pipe")
	var h: SteamHammerEntity = store.create(Vector3i(7 * B, 0, 0), "steam_hammer")
	assert_str(h.add_ingot(_hot("iron_ingot"))).is_equal("")
	h.set_recipe(recipes.recipes["smith_iron_plate"])
	_ticks(19)
	assert_object(h.boiler).is_null()
	_ticks(1)
	assert_object(h.boiler).is_same(b)
	_ticks(int(Settings.boiler_build_s * 20))
	assert_bool(h.powered()).is_true()
	_ticks(int(60.0 * 20))
	assert_int(h.strikes).is_greater(0)
	assert_str(h.ready.get("id", "")).is_equal("iron_plate")
	assert_bool(h.has_piece()).is_false()
	var out := h.take_piece()
	assert_str(out["id"]).is_equal("iron_plate")
	assert_dict(h.ready).is_empty()
	# too far: the ninth pipe reaches no steam
	var far: SteamHammerEntity = store.create(Vector3i(-(Settings.steam_pipe_max + 2) * B, 0, 0), "steam_hammer")
	for i in range(1, Settings.steam_pipe_max + 2):
		store.create(Vector3i(-i * B, 0, 0), "pipe")
	_ticks(20)
	assert_object(far.boiler).is_null()
	# copper pipes count double: four of them reach, five do not
	var near: SteamHammerEntity = store.create(Vector3i(0, 0, 9 * B), "steam_hammer")
	for i in range(5, 9):
		store.create(Vector3i(0, 0, i * B), "pipe").item = "copper_pipe"
	_ticks(20)
	assert_object(near.boiler).is_same(b)
	store.create(Vector3i(0, 0, 12 * B), "pipe").item = "copper_pipe"
	var past: SteamHammerEntity = store.create(Vector3i(0, 0, 13 * B), "steam_hammer")
	_ticks(20)
	assert_object(past.boiler).is_null()


func test_hammer_strike_moves_metal_toward_the_shape() -> void:
	var h: SteamHammerEntity = store.create(Vector3i.ZERO, "steam_hammer")
	h.add_ingot(_hot("iron_ingot"))
	h.set_recipe(recipes.recipes["smith_iron_plate"])
	var extra := h.extra_cells()
	for i in 200:
		if h.is_done():
			break
		h.strike()
	assert_bool(h.is_done()).is_true()
	assert_int(extra).is_greater(0)
	assert_int(h.strikes).is_less_equal(200)


# ---------- the steam sieve ----------

func test_steam_sieve_sifts_by_itself() -> void:
	var b := _boiler()
	store.create(Vector3i(5 * B, 0, 0), "pipe")
	var s: SteamSieveEntity = store.create(Vector3i(6 * B, 0, 0), "steam_sieve")
	var rng := RandomNumberGenerator.new()
	for i in 3:
		var res := s.use({"id": "gravel", "count": 64}, rng)
		assert_int(res["consume"]).is_equal(items.units("gravel"))
	assert_int(s.loads).is_equal(3)
	assert_str(s.use({"id": "sand", "count": 64}, rng)["msg"]).is_equal("It is sifting Gravel now.")
	_ticks(int(Settings.boiler_build_s * 20) + 20)
	_ticks(int(10.0 * 20))
	assert_int(s.loads).is_equal(0)
	assert_int(s.sifted).is_equal(3)
	assert_str(s.input).is_equal("")
	var found := s.found.size()
	var out := s.use({}, rng)
	assert_int(out["drops"].size()).is_equal(found)
	assert_array(s.found).is_empty()


# ---------- the wrench ----------

func test_wrench_recipe_and_turning() -> void:
	assert_bool(items.has("wrench")).is_true()
	assert_array(recipes.recipes["wrench"]["ingredients"]).contains_exactly_in_any_order(["iron_rod", "#stick"])
	var size := Vector3i(4, 1, 4)
	var c := WorkPalette.new_cells(size)
	c[WorkPalette.idx(size, 0, 0, 0)] = 7   # the min corner
	var once := WorkPalette.rotate_y(c, size, 1)
	assert_int(once[WorkPalette.idx(size, 3, 0, 0)]).is_equal(7)   # a quarter turn moves it to +x
	assert_int(once[WorkPalette.idx(size, 0, 0, 0)]).is_equal(0)
	assert_array(WorkPalette.rotate_y(c, size, 4)).is_equal(c)
	var b: BoilerEntity = store.create(Vector3i.ZERO, "boiler")
	b.facing = 3
	var d: Dictionary = JSON.parse_string(JSON.stringify(b.to_dict()))
	var b2 := BoilerEntity.new(items, recipes, Vector3i.ZERO)
	b2.load_from(d)
	assert_int(b2.facing).is_equal(3)
	assert_int(b2.view_parts()["solid"]["cells"].size()).is_equal(20 * 20 * 20)


# ---------- saves ----------

func test_saves_keep_steam_state() -> void:
	var b := _boiler()
	b.pressure = 0.7
	var h: SteamHammerEntity = store.create(Vector3i(6 * B, 0, 0), "steam_hammer")
	h.ready = {"id": "iron_plate", "count": 1}
	h.wear = 0.4
	var s: SteamSieveEntity = store.create(Vector3i(10 * B, 0, 0), "steam_sieve")
	s.use({"id": "gravel", "count": 64}, RandomNumberGenerator.new())
	s.found = [{"id": "flint", "count": 2}]
	store.create(Vector3i(12 * B, 0, 0), "pipe").axis = 1
	var other := BlockEntities.new(items, recipes)
	assert_bool(other.load_dict(JSON.parse_string(JSON.stringify(store.to_dict())))).is_true()
	var b2: BoilerEntity = other.at(Vector3i.ZERO)
	assert_float(b2.pressure).is_equal_approx(0.7, 0.001)
	assert_float(b2.water).is_equal_approx(b.water, 0.001)
	assert_bool(b2.is_lit()).is_true()
	var h2: SteamHammerEntity = other.at(Vector3i(6 * B, 0, 0))
	assert_str(h2.ready["id"]).is_equal("iron_plate")
	assert_float(h2.wear).is_equal_approx(0.4, 0.001)
	var s2: SteamSieveEntity = other.at(Vector3i(10 * B, 0, 0))
	assert_int(s2.loads).is_equal(1)
	assert_str(s2.found[0]["id"]).is_equal("flint")
	assert_str(other.at(Vector3i(12 * B, 0, 0)).kind).is_equal("pipe")
	assert_int(other.at(Vector3i(12 * B, 0, 0)).axis).is_equal(1)
