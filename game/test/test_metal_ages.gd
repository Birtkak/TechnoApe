extends GdUnitTestSuite

## The bronze and iron ages (ADR 0070): tin melted into molten copper on the firepit, bronze and iron
## tools, the bloomery's hot bloom, the forge (the only fire that heats iron) and the bronze anvil
## (the only anvil that takes it).

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities

const TOOLS := ["knife", "axe", "shovel", "hoe", "pickaxe", "hammer", "saw"]


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


func _head(m: String, t: String) -> String:
	return "%s_%s" % [m, "saw_blade" if t == "saw" else t + "_head"]


func _hot(id: String) -> Dictionary:
	return {"id": id, "count": 1, "hot_until": Inventory.clock_s + Settings.metal_cool_s}


# ---------- data ----------

func test_every_metal_tool_is_assembled_from_a_smithed_head() -> void:
	assert_array(recipes.errors).is_empty()
	for m in ["bronze", "iron"]:
		for t in TOOLS:
			var tool := "%s_%s" % [m, t]
			assert_int(int(items.get_def(tool).get("tier", -1))).override_failure_message(tool).is_equal(2 if m == "bronze" else 3)
			var a: Dictionary = recipes.recipes.get(tool, {})
			assert_str(a.get("type", "")).override_failure_message(tool).is_equal("assemble")
			assert_array(a["parts"]).is_equal([_head(m, t), "#stick"])
			assert_str(recipes.recipes.get("smith_" + _head(m, t), {}).get("input", "")).is_equal(m + "_ingot")
	# bronze casts in the copper molds (not the saw); iron never casts
	for r in recipes.of_type("cast"):
		assert_bool(String(r["output"]["id"]).begins_with("iron")).is_false()
	assert_array(recipes.makes("bronze_axe_head")).contains(["cast_bronze_axe_head", "smith_bronze_axe_head"])


func test_the_furnace_no_longer_smelts_ore() -> void:
	for ore in ["iron_ore", "copper_ore", "tin_ore"]:
		assert_dict(recipes.smelting_for(ore, "furnace")).override_failure_message(ore).is_empty()
	assert_dict(recipes.smelting_for("iron_ore", "bloomery")).is_not_empty()


func test_iron_ore_needs_a_bronze_pickaxe() -> void:
	var mining := MiningTable.load_default()
	assert_dict(mining.drops("iron_ore", items.get_def("copper_pickaxe"))).is_empty()
	assert_str(String(mining.drops("iron_ore", items.get_def("bronze_pickaxe")).get("id", ""))).is_equal("iron_ore")


# ---------- bronze ----------

func test_tin_melts_into_molten_copper() -> void:
	var f := store.create(Vector3i(0, 10, 0), "firepit") as FirepitEntity
	assert_int(f.use({"id": "crucible_copper", "count": 1}, items.get_def("crucible_copper"))["consume"]).is_equal(1)
	for i in 2:
		assert_int(f.use({"id": "tin_nugget", "count": 4}, items.get_def("tin_nugget"))["consume"]).is_equal(1)
	for i in 2:
		f.use({"id": "oak_log", "count": 64}, items.get_def("oak_log"))
	assert_bool(f.ignite()).is_true()
	_ticks(301)
	assert_str(f.use({}, {})["give"][0]["id"]).is_equal("crucible_bronze")
	assert_bool(f.inv.is_empty_slot(FirepitEntity.INPUT)).is_true()


func test_a_bronze_ingot_heats_on_the_firepit_and_smiths_on_stone() -> void:
	var f := store.create(Vector3i(0, 10, 0), "firepit") as FirepitEntity
	f.use({"id": "oak_log", "count": 64}, items.get_def("oak_log"))
	assert_int(f.use({"id": "bronze_ingot", "count": 2}, items.get_def("bronze_ingot"))["consume"]).is_equal(1)
	f.ignite()
	_ticks(int(Settings.metal_heat_s * 20.0) + 2)
	var hot: Dictionary = f.use({}, {})["give"][0]
	assert_float(Inventory.heat_of(hot)).is_equal_approx(1.0, 0.01)
	var a := store.create(Vector3i(8, 10, 0), "anvil") as AnvilEntity
	assert_str(a.add_ingot(hot)).is_equal("")
	assert_array(a.options().map(func(r: Dictionary) -> String: return r["id"])).contains(["smith_bronze_saw_blade"])


# ---------- iron ----------

func test_iron_does_not_heat_on_a_firepit_but_in_a_forge() -> void:
	var f := store.create(Vector3i(0, 10, 0), "firepit") as FirepitEntity
	var res := f.use({"id": "iron_ingot", "count": 1}, items.get_def("iron_ingot"))
	assert_int(res["consume"]).is_equal(0)
	assert_str(res["msg"]).contains("forge")
	var g := store.create(Vector3i(8, 10, 0), "forge") as ForgeEntity
	assert_int(g.use({"id": "oak_log", "count": 64}, items.get_def("oak_log"))["consume"]).is_equal(0)   # charcoal or coal only
	assert_int(g.use({"id": "charcoal", "count": 4}, items.get_def("charcoal"))["consume"]).is_equal(1)
	assert_int(g.use({"id": "crucible", "count": 1}, items.get_def("crucible"))["consume"]).is_equal(0)   # no vessel
	assert_int(g.use({"id": "iron_ingot", "count": 1}, items.get_def("iron_ingot"))["consume"]).is_equal(1)
	assert_bool(g.ignite()).is_true()
	_ticks(int(Settings.metal_heat_s * 20.0) + 2)
	var hot: Dictionary = g.use({}, {})["give"][0]
	assert_str(hot["id"]).is_equal("iron_ingot")
	assert_float(Inventory.heat_of(hot)).is_equal_approx(1.0, 0.01)
	assert_array(g.contents().map(func(s: Dictionary) -> String: return s["id"])).contains(["forge"])


## Bug report 2026-10-27_18-02-35 (ADR 0087): two ingots on the forge, both drawn on the coals.
func test_a_forge_holds_two_ingots() -> void:
	var g := store.create(Vector3i(0, 10, 0), "forge") as ForgeEntity
	var ingot := items.get_def("iron_ingot")
	assert_int(g.use({"id": "iron_ingot", "count": 5}, ingot)["consume"]).is_equal(1)
	assert_int(g.use({"id": "iron_ingot", "count": 4}, ingot)["consume"]).is_equal(1)
	var res := g.use({"id": "iron_ingot", "count": 3}, ingot)
	assert_int(res["consume"]).is_equal(0)
	assert_str(res["msg"]).contains("no more")
	assert_int(g.view_parts()["items"].size()).is_equal(2)


## Bug report 2026-10-27_18-01-05 (ADR 0087): sneaking loads as much of the held stack as fits.
func test_sneak_loads_as_much_as_fits() -> void:
	var b := store.create(Vector3i(0, 10, 0), "bloomery") as BloomeryEntity
	var res := b.use({"id": "iron_nugget", "count": 64}, items.get_def("iron_nugget"), false, true)
	assert_int(res["consume"]).is_equal(64)
	res = b.use({"id": "charcoal", "count": 20}, items.get_def("charcoal"), false, true)
	assert_int(res["consume"]).is_equal(Settings.firepit_fuel_max)
	var g := store.create(Vector3i(8, 10, 0), "forge") as ForgeEntity
	assert_int(g.use({"id": "iron_ingot", "count": 9}, items.get_def("iron_ingot"), false, true)["consume"]).is_equal(2)


func test_the_bloomery_makes_a_hot_bloom() -> void:
	var b := store.create(Vector3i(0, 10, 0), "bloomery") as BloomeryEntity
	for i in 8:
		assert_int(b.use({"id": "iron_nugget", "count": 8}, items.get_def("iron_nugget"))["consume"]).is_equal(1)
	assert_int(b.use({"id": "copper_ingot", "count": 1}, items.get_def("copper_ingot"))["consume"]).is_equal(0)   # heats nothing
	for i in 2:
		assert_int(b.use({"id": "charcoal", "count": 4}, items.get_def("charcoal"))["consume"]).is_equal(1)
	assert_bool(b.ignite()).is_true()
	_ticks(2399)
	assert_bool(b.inv.is_empty_slot(FirepitEntity.OUTPUT)).is_true()
	_ticks(2)
	var bloom: Dictionary = b.use({}, {})["give"][0]
	assert_str(bloom["id"]).is_equal("iron_bloom")
	assert_float(Inventory.heat_of(bloom)).is_greater(0.9)
	assert_bool(b.inv.is_empty_slot(FirepitEntity.INPUT)).is_true()


# ---------- the look-at panel and cues (ADR 0078) ----------

func test_a_forge_says_what_is_in_it() -> void:
	var g := store.create(Vector3i(0, 10, 0), "forge") as ForgeEntity
	assert_str(g.look_lines()[0]).contains("Not lit")
	g.use({"id": "charcoal", "count": 4}, items.get_def("charcoal"))
	g.use({"id": "charcoal", "count": 4}, items.get_def("charcoal"))
	g.use({"id": "iron_ingot", "count": 1}, items.get_def("iron_ingot"))
	g.ignite()
	var text := "\n".join(g.look_lines())
	assert_str(text).contains("Burning")
	assert_str(text).contains("Fuel: 1 Charcoal")   # one of the two burns now
	assert_str(text).contains("On the fire: 1 Iron Ingot")
	assert_str(text).contains("°C")


func test_an_anvil_says_how_far_the_piece_is_and_hisses_when_it_cools() -> void:
	var a := store.create(Vector3i(0, 10, 0), "anvil") as AnvilEntity
	assert_str(a.look_lines()[0]).contains("Empty")
	a.add_ingot(_hot("copper_ingot"))
	a.set_recipe(a.options()[0])
	var text := "\n".join(a.look_lines())
	assert_str(text).contains("Hot")
	assert_str(text).contains("extra cells")
	var cooled := []
	a.cooled.connect(func(p: Vector3i) -> void: cooled.append(p))
	_ticks(1)
	_ticks(int(Settings.metal_cool_s * 20.0))
	assert_array(cooled).is_equal([Vector3i(0, 10, 0)])
	assert_str("\n".join(a.look_lines())).contains("Too cold")


func test_only_the_bronze_anvil_takes_iron() -> void:
	var stone := store.create(Vector3i(0, 10, 0), "anvil") as AnvilEntity
	assert_str(stone.add_ingot(_hot("iron_ingot"))).contains("bronze anvil")
	assert_bool(stone.has_piece()).is_false()
	var piece := {"id": "iron_work_piece", "count": 1, "work": {"metal": "iron_ingot", "recipe": "", "heights": Array(stone.heights), "output": ""}}
	assert_str(stone.put_piece(piece)).contains("bronze anvil")
	var bronze := store.create(Vector3i(8, 10, 0), "bronze_anvil") as AnvilEntity
	assert_str(bronze.add_ingot(_hot("iron_ingot"))).is_equal("")
	assert_array(bronze.options().map(func(r: Dictionary) -> String: return r["id"])).contains(["smith_iron_pickaxe_head", "smith_iron_saw_blade"])
	assert_array(bronze.contents().map(func(s: Dictionary) -> String: return s["id"])).contains(["bronze_anvil", "iron_work_piece"])


func test_a_bloom_is_beaten_into_an_ingot() -> void:
	var a := store.create(Vector3i(0, 10, 0), "bronze_anvil") as AnvilEntity
	assert_str(a.add_ingot(_hot("iron_bloom"))).is_equal("")
	var opts: Array = a.options().map(func(r: Dictionary) -> String: return r["id"])
	assert_array(opts).is_equal(["smith_iron_ingot"])
	a.set_recipe(recipes.recipes["smith_iron_ingot"])
	# the lump (columns 2..5, rows 2..5, two high) goes into rows 3..4, then out to both ends
	for x in range(2, 6):
		for i in 2:
			a.hit(x, 2, Vector2i(0, 1))
			a.hit(x, 5, Vector2i(0, -1))
	var result := ""
	for z in [3, 4]:
		for x in [2, 2, 2, 1, 1, 3, 3, 2]:
			result = a.hit(x, z, Vector2i(-1, 0))
		for x in [5, 5, 5, 6, 6, 4, 4, 5]:
			result = a.hit(x, z, Vector2i(1, 0))
	assert_str(result).is_equal("done")
	assert_dict(a.finish()).is_equal({"id": "iron_ingot", "count": 1})


func test_new_stations_save_and_load() -> void:
	var b := store.create(Vector3i(0, 10, 0), "bloomery") as BloomeryEntity
	b.inv.set_slot(FirepitEntity.INPUT, {"id": "loose_iron_ore", "count": 4})
	var a := store.create(Vector3i(8, 10, 0), "bronze_anvil") as AnvilEntity
	a.add_ingot(_hot("bronze_ingot"))
	store.create(Vector3i(16, 10, 0), "forge")
	var other := BlockEntities.new(items, recipes)
	assert_bool(other.load_dict(store.to_dict())).is_true()
	assert_object(other.at(Vector3i(0, 10, 0))).is_instanceof(BloomeryEntity)
	assert_object(other.at(Vector3i(16, 10, 0))).is_instanceof(ForgeEntity)
	var a2 := other.at(Vector3i(8, 10, 0)) as AnvilEntity
	assert_str(a2.kind).is_equal("bronze_anvil")
	assert_int(a2.tier).is_equal(1)
	assert_str(a2.metal).is_equal("bronze_ingot")
