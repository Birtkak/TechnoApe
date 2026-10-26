extends GdUnitTestSuite

## Headless tests for station blocks (ADR 0007): material/atlas data, chest
## and furnace containers with vanilla shift-click order, furnace ticks
## (fuel, cook time, lava bucket, output full, decay), block entity store.

var items: ItemRegistry
var recipes: RecipeRegistry
var mining: MiningTable
var p: PlayerInventory
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	mining = MiningTable.load_default()
	p = PlayerInventory.new(items, recipes)
	store = BlockEntities.new(items, recipes)


func _furnace() -> FurnaceEntity:
	return store.create(Vector3i(1, 2, 3), "furnace") as FurnaceEntity


func _run(f: FurnaceEntity, ticks: int) -> int:
	var flips := 0
	for i in ticks:
		if f.tick():
			flips += 1
	return flips


# ---------- data ----------

func test_station_materials_and_items() -> void:
	for n in ["crafting_table", "furnace", "furnace_lit", "chest", "planks"]:
		var id := -1
		for i in GameMaterials.defs().size():
			if GameMaterials.defs()[i]["name"] == n:
				id = i + 1
		assert_int(id).override_failure_message("material %s missing" % n).is_greater(0)
		assert_bool(mining.is_minable(n)).override_failure_message("%s not minable" % n).is_true()
	assert_int(GameMaterials.CRAFTING_TABLE).is_equal(32)
	assert_str(GameMaterials.name_of(GameMaterials.FURNACE_LIT)).is_equal("furnace_lit")
	assert_str(GameMaterials.name_of(GameMaterials.PLANKS)).is_equal("planks")
	for it in ["crafting_table", "furnace", "chest", "oak_planks"]:
		assert_int(ItemIcon.material_id_for(items, it)).override_failure_message("%s does not place" % it).is_greater(0)
	assert_str(mining.drops("furnace_lit", items.get_def("flint_pickaxe")).get("id", "")).is_equal("furnace")
	assert_dict(mining.drops("furnace", {})).is_empty()   # needs a pickaxe like vanilla


func test_atlas_covers_every_tile() -> void:
	var n := GameMaterials.ATLAS_TILES.x * GameMaterials.ATLAS_TILES.y
	for d in GameMaterials.defs():
		for t in d["tiles"]:
			assert_int(int(t)).is_less(n)
	var img: Texture2D = load("res://assets/textures/atlas.png")
	assert_vector(img.get_size()).is_equal(Vector2(GameMaterials.ATLAS_TILES * 16))


# ---------- chest ----------

func test_chest_shift_click_in_and_out_vanilla_order() -> void:
	var c: ChestEntity = store.create(Vector3i.ZERO, "chest")
	p.open_container(c)
	p.inv.set_slot(12, {"id": "stick", "count": 40})
	p.click_slot(12, MOUSE_BUTTON_LEFT, true)
	assert_dict(c.inv.get_slot(0)).is_equal({"id": "stick", "count": 40})
	assert_bool(p.inv.is_empty_slot(12)).is_true()
	# back out: reverse menu order fills the last hotbar slot first
	p.click_container_slot(0, MOUSE_BUTTON_LEFT, true)
	assert_dict(p.inv.get_slot(8)).is_equal({"id": "stick", "count": 40})
	# merges into an existing stack before taking an empty slot
	c.inv.set_slot(3, {"id": "stick", "count": 30})
	p.click_container_slot(3, MOUSE_BUTTON_LEFT, true)
	assert_dict(p.inv.get_slot(8)).is_equal({"id": "stick", "count": 64})
	assert_dict(p.inv.get_slot(7)).is_equal({"id": "stick", "count": 6})


func test_chest_full_keeps_stack_in_player() -> void:
	var c: ChestEntity = store.create(Vector3i.ZERO, "chest")
	for i in ChestEntity.SIZE:
		c.inv.set_slot(i, {"id": "stone", "count": 64})
	p.open_container(c)
	p.inv.set_slot(0, {"id": "dirt", "count": 5})
	p.click_slot(0, MOUSE_BUTTON_LEFT, true)
	assert_dict(p.inv.get_slot(0)).is_equal({"id": "dirt", "count": 5})   # no fallback to main like vanilla


func test_chest_cursor_clicks() -> void:
	var c: ChestEntity = store.create(Vector3i.ZERO, "chest")
	p.open_container(c)
	c.inv.set_slot(5, {"id": "coal", "count": 9})
	p.click_container_slot(5, MOUSE_BUTTON_RIGHT, false)
	assert_dict(p.cursor).is_equal({"id": "coal", "count": 5})
	p.click_container_slot(6, MOUSE_BUTTON_LEFT, false)
	assert_dict(c.inv.get_slot(6)).is_equal({"id": "coal", "count": 5})
	assert_dict(p.cursor).is_empty()
	var dropped := p.close_screen()
	assert_array(dropped).is_empty()
	assert_object(p.container).is_null()


# ---------- furnace ----------

func test_furnace_chars_logs_with_coal() -> void:
	var f := _furnace()
	f.inv.set_slot(FurnaceEntity.INPUT, {"id": "oak_log", "count": 3 * items.units("oak_log")})   # three blocks of log voxels (ADR 0034)
	f.inv.set_slot(FurnaceEntity.FUEL, {"id": "coal", "count": 1})
	assert_int(_run(f, 1)).is_equal(1)      # lights on the first tick
	assert_bool(f.is_lit()).is_true()
	assert_bool(f.inv.is_empty_slot(FurnaceEntity.FUEL)).is_true()
	_run(f, 198)
	assert_bool(f.inv.is_empty_slot(FurnaceEntity.OUTPUT)).is_true()
	_run(f, 1)                              # tick 200: first ingot (vanilla 10 s)
	assert_dict(f.inv.get_slot(FurnaceEntity.OUTPUT)).is_equal({"id": "charcoal", "count": 1})
	_run(f, 400)
	assert_dict(f.inv.get_slot(FurnaceEntity.OUTPUT)).is_equal({"id": "charcoal", "count": 3})
	assert_bool(f.inv.is_empty_slot(FurnaceEntity.INPUT)).is_true()
	assert_bool(f.is_lit()).is_true()       # coal burns 1600 ticks = 8 items, keeps burning empty
	assert_int(_run(f, 1001)).is_equal(1)   # and goes out once, on tick 1601
	assert_bool(f.is_lit()).is_false()


func test_furnace_does_not_burn_fuel_without_smeltable_input() -> void:
	var f := _furnace()
	f.inv.set_slot(FurnaceEntity.INPUT, {"id": "dirt", "count": 3})
	f.inv.set_slot(FurnaceEntity.FUEL, {"id": "coal", "count": 2})
	_run(f, 50)
	assert_bool(f.is_lit()).is_false()
	assert_dict(f.inv.get_slot(FurnaceEntity.FUEL)).is_equal({"id": "coal", "count": 2})


func test_furnace_lava_bucket_leaves_bucket() -> void:
	var f := _furnace()
	f.inv.set_slot(FurnaceEntity.INPUT, {"id": "sand", "count": items.units("sand")})
	f.inv.set_slot(FurnaceEntity.FUEL, {"id": "lava_bucket", "count": 1})
	_run(f, 1)
	assert_dict(f.inv.get_slot(FurnaceEntity.FUEL)).is_equal({"id": "bucket", "count": 1})
	assert_int(f.lit_time).is_equal(20000)


func test_furnace_output_full_stops_and_progress_decays() -> void:
	var f := _furnace()
	f.inv.set_slot(FurnaceEntity.INPUT, {"id": "oak_log", "count": 5 * items.units("oak_log")})
	f.inv.set_slot(FurnaceEntity.FUEL, {"id": "stick", "count": 1})   # 100 ticks of fire
	_run(f, 100)
	assert_int(f.cook).is_equal(100)           # half an item per stick, like vanilla
	_run(f, 10)                                # fire out on tick 101, then -2 per tick
	assert_bool(f.is_lit()).is_false()
	assert_int(f.cook).is_equal(80)
	f.inv.set_slot(FurnaceEntity.OUTPUT, {"id": "glass", "count": 1})   # different item blocks the output
	f.inv.set_slot(FurnaceEntity.FUEL, {"id": "coal", "count": 1})
	_run(f, 20)
	assert_bool(f.is_lit()).is_false()         # cannot smelt, so the coal is not lit
	assert_dict(f.inv.get_slot(FurnaceEntity.FUEL)).is_equal({"id": "coal", "count": 1})


func test_furnace_input_change_resets_progress() -> void:
	var f := _furnace()
	f.inv.set_slot(FurnaceEntity.INPUT, {"id": "oak_log", "count": 2 * items.units("oak_log")})
	f.inv.set_slot(FurnaceEntity.FUEL, {"id": "coal", "count": 1})
	_run(f, 50)
	assert_int(f.cook).is_equal(50)
	f.inv.set_slot(FurnaceEntity.INPUT, {"id": "sand", "count": 2 * items.units("sand")})
	assert_int(f.cook).is_equal(0)


func test_furnace_slot_rules_and_shift_click() -> void:
	var f := _furnace()
	p.open_container(f)
	p.inv.set_slot(9, {"id": "sand", "count": 4})
	p.inv.set_slot(10, {"id": "coal", "count": 3})
	p.inv.set_slot(11, {"id": "oak_log", "count": 2})   # smeltable and fuel: vanilla prefers the input
	p.inv.set_slot(12, {"id": "dirt", "count": 2})
	p.click_slot(9, MOUSE_BUTTON_LEFT, true)
	p.click_slot(10, MOUSE_BUTTON_LEFT, true)
	assert_dict(f.inv.get_slot(FurnaceEntity.INPUT)).is_equal({"id": "sand", "count": 4})
	assert_dict(f.inv.get_slot(FurnaceEntity.FUEL)).is_equal({"id": "coal", "count": 3})
	p.click_slot(11, MOUSE_BUTTON_LEFT, true)            # input taken by sand: stays put
	assert_dict(p.inv.get_slot(11)).is_equal({"id": "oak_log", "count": 2})
	p.click_slot(12, MOUSE_BUTTON_LEFT, true)            # neither: main -> hotbar
	assert_dict(p.inv.get_slot(0)).is_equal({"id": "dirt", "count": 2})
	# fuel slot refuses dirt, output refuses everything
	p.cursor = {"id": "dirt", "count": 2}
	p.inv.set_slot(0, {})
	p.click_container_slot(FurnaceEntity.FUEL, MOUSE_BUTTON_LEFT, false)
	assert_dict(f.inv.get_slot(FurnaceEntity.FUEL)).is_equal({"id": "coal", "count": 3})
	p.click_container_slot(FurnaceEntity.OUTPUT, MOUSE_BUTTON_LEFT, false)
	assert_bool(f.inv.is_empty_slot(FurnaceEntity.OUTPUT)).is_true()
	# output into a matching cursor takes as much as fits
	p.cursor = {"id": "iron_ingot", "count": 60}
	f.inv.set_slot(FurnaceEntity.OUTPUT, {"id": "iron_ingot", "count": 10})
	p.click_container_slot(FurnaceEntity.OUTPUT, MOUSE_BUTTON_LEFT, false)
	assert_dict(p.cursor).is_equal({"id": "iron_ingot", "count": 64})
	assert_dict(f.inv.get_slot(FurnaceEntity.OUTPUT)).is_equal({"id": "iron_ingot", "count": 6})


# ---------- store ----------

func test_store_remove_returns_contents_and_ticks_furnaces() -> void:
	var c: ChestEntity = store.create(Vector3i(5, 5, 5), "chest")
	c.inv.set_slot(2, {"id": "coal", "count": 7})
	var f := _furnace()
	f.inv.set_slot(FurnaceEntity.INPUT, {"id": "sand", "count": items.units("sand")})
	f.inv.set_slot(FurnaceEntity.FUEL, {"id": "coal", "count": 1})
	var flipped := store.tick()
	assert_array(flipped).contains_exactly([Vector3i(1, 2, 3)])
	assert_object(store.get_or_create(Vector3i(5, 5, 5), "chest")).is_same(c)
	assert_array(store.remove(Vector3i(5, 5, 5))).contains_exactly([{"id": "coal", "count": 7}])
	assert_bool(store.has(Vector3i(5, 5, 5))).is_false()
	assert_int(store.size()).is_equal(1)
	var s := store.summary()
	assert_int(s["count"]).is_equal(1)
	assert_str(JSON.stringify(s)).is_not_empty()


func test_crafting_table_recipes_need_the_table() -> void:
	var grid := ["oak_planks", "oak_planks", "oak_planks", "oak_planks", "", "oak_planks", "oak_planks", "oak_planks", "oak_planks"]
	assert_dict(recipes.match_grid(grid, "")).is_empty()
	assert_str(recipes.match_grid(grid, "crafting_table").get("id", "")).is_equal("chest")
	assert_str(recipes.smelting_for("sand").get("id", "")).is_equal("smelt_glass")
	assert_dict(recipes.smelting_for("dirt")).is_empty()


func test_save_and_load_round_trip() -> void:
	var c: ChestEntity = store.create(Vector3i(5, 6, 7), "chest")
	c.inv.set_slot(26, {"id": "coal", "count": 7})
	var f := _furnace()
	f.inv.set_slot(FurnaceEntity.INPUT, {"id": "oak_log", "count": 4 * items.units("oak_log")})
	f.inv.set_slot(FurnaceEntity.FUEL, {"id": "coal", "count": 2})
	_run(f, 150)
	assert_bool(store.dirty).is_true()
	var path := "user://test_blocks_roundtrip.json"
	assert_bool(store.save_file(path)).is_true()
	assert_bool(store.dirty).is_false()
	var other := BlockEntities.new(items, recipes)
	assert_bool(other.load_file(path)).is_true()
	assert_int(other.size()).is_equal(2)
	var c2: ChestEntity = other.at(Vector3i(5, 6, 7))
	assert_dict(c2.inv.get_slot(26)).is_equal({"id": "coal", "count": 7})
	var f2: FurnaceEntity = other.at(Vector3i(1, 2, 3))
	assert_int(f2.cook).is_equal(150)
	assert_int(f2.lit_time).is_equal(f.lit_time)
	assert_dict(f2.inv.get_slot(FurnaceEntity.FUEL)).is_equal({"id": "coal", "count": 1})
	_run(f2, 50)                                  # carries on where it stopped
	assert_dict(f2.inv.get_slot(FurnaceEntity.OUTPUT)).is_equal({"id": "charcoal", "count": 1})
	assert_bool(other.load_dict({"version": 99})).is_false()
	DirAccess.remove_absolute(ProjectSettings.globalize_path(path))
