extends GdUnitTestSuite

## The steam age, stage 2 and the turbine (ADR 0099): fire bricks and steel data, the blast furnace
## that only works under steam, the steel boiler's valve (and its rusty burst), the ore crusher's two
## crushed ore per block, the water tank feeding a boiler over pipes, the turbine, saves.

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


## Ticks with the small boiler vented at working pressure, as a player watching the gauge would.
func _ticks(n: int, b: BoilerEntity = null) -> void:
	for i in n:
		Inventory.clock_s += BlockEntities.TICK_S
		if b != null and b.pressure > 1.0:
			b.pressure = 1.0
		store.tick()


## A lit boiler with a full tank at the origin (5 blocks wide), a pipe at 5B: machines go at 6B.
func _boiler(item := "small_boiler") -> BoilerEntity:
	var b: BoilerEntity = store.create(Vector3i.ZERO, "boiler")
	b.item = item
	b.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	for i in 4:
		b.use({"id": "water_bucket", "count": 1}, items.get_def("water_bucket"))
	assert_bool(b.ignite()).is_true()
	store.create(Vector3i(5 * B, 0, 0), "pipe")
	return b


func test_items_and_recipes_exist() -> void:
	for id in ["fire_clay", "fire_brick", "steel_ingot", "steel_pickaxe", "steel_plate", "blast_furnace", "steel_boiler",
			"ore_crusher", "water_tank", "crushed_iron_ore", "crushed_copper_ore", "crushed_tin_ore", "copper_wire", "steam_turbine"]:
		assert_bool(items.has(id)).override_failure_message(id).is_true()
	for id in ["fire_clay", "kiln_fire_brick", "blast_furnace", "blast_steel", "smith_steel_pickaxe_head", "steel_pickaxe",
			"smith_steel_plate", "steel_boiler", "ore_crusher", "water_tank", "bloom_crushed_iron_ore", "melt_crushed_copper_ore",
			"alloy_bronze_crushed_tin_ore", "smith_copper_wire", "steam_turbine"]:
		assert_bool(recipes.recipes.has(id)).override_failure_message(id).is_true()
	assert_int(int(items.get_def("steel_pickaxe")["tier"])).is_equal(4)
	for ore in ["iron_ore", "copper_ore", "tin_ore"]:
		assert_str(items.get_def(ore).get("crushed", "")).is_equal("crushed_" + ore)
	assert_int(AnvilEntity.ingots_for(recipes.recipes["smith_copper_wire"])).is_equal(1)


func test_helper_items_exist() -> void:
	for id in ["iron_gear", "glass_lens", "copper_coil", "steam_cell", "miners_helmet", "travelers_backpack", "steam_drill",
			"boiler_suit", "jetpack", "surveyors_stake", "compressor"]:
		assert_bool(items.has(id)).override_failure_message(id).is_true()
		assert_bool(recipes.recipes.has("smith_" + id if id == "iron_gear" else id)).override_failure_message(id).is_true()
	assert_bool(TorchLights.light_of(items, "miners_helmet").is_empty()).is_false()
	assert_int(int(items.get_def("travelers_backpack")["bag_slots"])).is_equal(2 * PlayerInventory.BAG)
	assert_str(Clothing.slot_of(items.get_def("jetpack"))).is_equal("back")
	assert_bool("back" in Clothing.SLOTS).is_true()
	assert_int(AnvilEntity.ingots_for(recipes.recipes["smith_iron_gear"])).is_equal(1)


func test_blast_furnace_makes_steel_only_under_steam() -> void:
	var f: BlastFurnaceEntity = store.create(Vector3i(6 * B, 0, 0), "blast_furnace")
	f.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	var r := f.use({"id": "iron_ingot", "count": 1}, items.get_def("iron_ingot"))
	assert_int(r["consume"]).is_equal(1)
	assert_bool(f.ignite()).is_true()
	_ticks(100)
	assert_int(f.cook).is_equal(0)   # lit, no blast: the charge waits
	assert_str(f.look_lines()[0]).is_equal("No steam: lay pipes from a boiler to blow it")
	var b := _boiler()
	_ticks(int(recipes.recipes["blast_steel"]["ticks"]) + 60, b)
	assert_object(f.boiler).is_same(b)
	var out := f.inv.get_slot(FirepitEntity.OUTPUT)
	assert_str(out.get("id", "")).is_equal("steel_ingot")
	assert_float(Inventory.heat_of(out)).is_greater(0.5)


func test_steel_boiler_valve_holds_and_sticks_with_rust() -> void:
	var b := _boiler("steel_boiler")
	assert_float(b.strength()).is_equal(Settings.steel_boiler_x)
	_ticks(60 * 20)
	assert_float(b.pressure).is_equal(BoilerEntity.STEEL_VALVE)   # the valve blows off, no burst
	assert_bool(b.screaming()).is_false()
	assert_array(b.contents()).contains([{"id": "steel_boiler", "count": 1}])
	b.wear = 0.6
	var bursts := []
	b.burst.connect(func(p: Vector3i) -> void: bursts.append(p))
	_ticks(60 * 20)
	assert_array(bursts).is_equal([Vector3i.ZERO])


func test_ore_crusher_gives_two_crushed_per_block() -> void:
	var b := _boiler()
	var c: OreCrusherEntity = store.create(Vector3i(6 * B, 0, 0), "ore_crusher")
	assert_str(c.use({"id": "gravel", "count": 1}, null)["msg"]).is_equal("The crusher takes blocks of ore.")
	var r := c.use({"id": "iron_ore", "count": 1}, null)
	assert_int(r["consume"]).is_equal(items.units("iron_ore"))
	r = c.use({"id": "copper_ore", "count": 1}, null)
	assert_str(r["msg"]).is_equal("It is crushing Iron Ore now.")
	_ticks(int(Settings.ore_crusher_crush_s * 20) + 60, b)
	assert_int(c.loads).is_equal(0)
	assert_array(c.use({}, null)["drops"]).is_equal([{"id": "crushed_iron_ore", "count": 2}])
	assert_bool(recipes.smelting_for("crushed_iron_ore", "bloomery").is_empty()).is_false()


func test_water_tank_feeds_the_boiler_over_pipes() -> void:
	var b := _boiler()
	var t: TankEntity = store.create(Vector3i(6 * B, 0, 0), "tank")
	for i in 3:
		var r := t.use(items.get_def("water_bucket"))
		assert_array(r["give"]).is_equal([{"id": "bucket", "count": 1}])
	assert_int(t.buckets).is_equal(3)
	b.water = 0.0
	_ticks(40, b)   # two feeds
	assert_float(b.water).is_equal_approx(2 * Settings.boiler_bucket_fill, 0.01)
	assert_int(t.buckets).is_equal(1)
	assert_array(t.use(items.get_def("bucket"))["give"]).is_equal([{"id": "water_bucket", "count": 1}])
	assert_str(t.use(items.get_def("bucket"))["msg"]).is_equal("The tank is empty.")


func test_turbine_spins_on_steam() -> void:
	var t: TurbineEntity = store.create(Vector3i(6 * B, 0, 0), "turbine")
	_ticks(20)
	assert_float(t.rpm).is_equal(0.0)
	var b := _boiler()
	_ticks(20 * 25, b)
	assert_float(t.rpm).is_greater(0.0)
	assert_int(t.spin).is_greater(0)
	assert_str(t.use({})["msg"]).contains("rpm")


func test_compressor_charges_a_jetpack() -> void:
	var c: CompressorEntity = store.create(Vector3i(6 * B, 0, 0), "compressor")
	var pack := {"id": "jetpack", "count": 1}
	assert_str(c.use(pack)["msg"]).is_equal("The air tank is empty: it needs steam to pump.")
	var b := _boiler()
	_ticks(int(Settings.compressor_fill_s * 30), b)   # the boiler needs boiler_build_s to come up first
	assert_float(c.air).is_equal(1.0)
	assert_str(c.use({"id": "stick", "count": 1})["msg"]).contains("jetpack")
	assert_str(c.use(pack)["msg"]).is_equal("The jetpack is charged to 100%.")
	assert_float(float(pack["air"])).is_equal(1.0)
	assert_float(c.air).is_equal(0.0)
	assert_str(c.use(pack)["msg"]).is_equal("The air tank is empty: it needs steam to pump.")
	assert_int(MiningTable.dig_cap({})).is_equal(4)


func test_saves_keep_stage_2_state() -> void:
	var b := _boiler("steel_boiler")
	var t: TankEntity = store.create(Vector3i(6 * B, 0, 0), "tank")
	t.buckets = 5
	var c: OreCrusherEntity = store.create(Vector3i(0, 0, 6 * B), "ore_crusher")
	c.use({"id": "tin_ore", "count": 1}, null)
	var d := store.to_dict()
	var again := BlockEntities.new(items, recipes)
	assert_bool(again.load_dict(JSON.parse_string(JSON.stringify(d)))).is_true()
	assert_str(again.at(Vector3i.ZERO).item).is_equal("steel_boiler")
	assert_int(again.at(Vector3i(6 * B, 0, 0)).buckets).is_equal(5)
	assert_str(again.at(Vector3i(0, 0, 6 * B)).input).is_equal("tin_ore")
