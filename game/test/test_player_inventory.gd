extends GdUnitTestSuite

## Headless tests for the player model (ADR 0005): hotbar selection, pickup
## order, shift-click between hotbar and main, crafting output, buckets,
## health; and the mining table against the material list and items.

var items: ItemRegistry
var recipes: RecipeRegistry
var mining: MiningTable
var p: PlayerInventory


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	mining = MiningTable.load_default()
	p = PlayerInventory.new(items, recipes)


# ---------- mining table data ----------

func test_mining_table_loads() -> void:
	assert_array(mining.errors).is_empty()
	assert_int(mining.materials.size()).is_greater(10)


func test_mining_table_names_exist() -> void:
	var names := []
	for d in GameMaterials.defs():
		names.append(d["name"])
	for m in mining.materials:
		assert_bool(m in names).override_failure_message("unknown material in mining.json: %s" % m).is_true()
		var e: Dictionary = mining.materials[m]
		if e.has("drops"):
			assert_bool(items.has(e["drops"])).override_failure_message("unknown drop item %s for %s" % [e["drops"], m]).is_true()


func test_block_items_reference_materials() -> void:
	var names := []
	for d in GameMaterials.defs():
		names.append(d["name"])
	for id in items.all_ids():
		var d := items.get_def(id)
		if d.has("material"):
			assert_bool(d["material"] in names).override_failure_message("item %s places unknown material %s" % [id, d["material"]]).is_true()


func test_break_times_follow_minecraft() -> void:
	# stone needs a pickaxe: by hand 5x hardness and no drop, wooden pickaxe hardness*1.5/2
	assert_float(mining.break_time("stone")).is_equal_approx(7.5, 0.01)
	assert_dict(mining.drops("stone")).is_empty()
	var wood_pick := items.get_def("wooden_pickaxe")
	assert_float(mining.break_time("stone", wood_pick)).is_equal_approx(1.125, 0.01)
	assert_dict(mining.drops("stone", wood_pick)).is_equal({"id": "cobblestone", "count": 1})
	# dirt by hand drops; a pickaxe does not help
	assert_float(mining.break_time("dirt")).is_equal_approx(0.75, 0.01)
	assert_float(mining.break_time("dirt", wood_pick)).is_equal_approx(0.75, 0.01)
	assert_dict(mining.drops("dirt")).is_equal({"id": "dirt", "count": 1})
	# iron needs tier 1: wooden pickaxe is slow and yields nothing, stone pickaxe works
	assert_dict(mining.drops("iron_ore", wood_pick)).is_empty()
	assert_dict(mining.drops("iron_ore", items.get_def("stone_pickaxe"))).is_equal({"id": "iron_ore", "count": 1})
	# liquids and unknown materials are unbreakable
	assert_float(mining.break_time("water")).is_equal(-1.0)
	assert_float(mining.break_time("bedrock")).is_equal(-1.0)
	assert_float(mining.break_time("tall_grass")).is_equal(0.0)


# ---------- hotbar ----------

func test_select_and_scroll_wrap() -> void:
	p.select(8)
	p.scroll(1)
	assert_int(p.selected).is_equal(0)
	p.scroll(-1)
	assert_int(p.selected).is_equal(8)
	p.select(3)
	assert_int(p.selected).is_equal(3)


func test_pickup_fills_hotbar_first_and_merges() -> void:
	assert_int(p.pickup("dirt", 70)).is_equal(0)
	assert_dict(p.inv.get_slot(0)).is_equal({"id": "dirt", "count": 64})
	assert_dict(p.inv.get_slot(1)).is_equal({"id": "dirt", "count": 6})
	# a stack in main merges before a new hotbar slot is used
	p.inv.set_slot(20, {"id": "sand", "count": 10})
	p.pickup("sand", 5)
	assert_dict(p.inv.get_slot(20)).is_equal({"id": "sand", "count": 15})
	assert_bool(p.inv.is_empty_slot(2)).is_true()


func test_pickup_leftover_when_full() -> void:
	for i in PlayerInventory.SIZE:
		p.inv.set_slot(i, {"id": "stone", "count": 64})
	assert_int(p.pickup("stone", 3)).is_equal(3)
	assert_int(p.pickup("dirt", 1)).is_equal(1)


func test_consume_and_drop_held() -> void:
	p.pickup("sand", 3)
	assert_int(p.consume_held(1)).is_equal(1)
	assert_dict(p.held()).is_equal({"id": "sand", "count": 2})
	assert_dict(p.drop_held(false)).is_equal({"id": "sand", "count": 1})
	assert_dict(p.drop_held(true)).is_equal({"id": "sand", "count": 1})
	assert_dict(p.held()).is_empty()
	assert_dict(p.drop_held(true)).is_empty()


func test_bucket_fills_and_empties() -> void:
	p.pickup("bucket", 2)
	var def := p.held_def()
	assert_str(def["fills"]["water"]).is_equal("water_bucket")
	assert_int(p.replace_held_one("water_bucket")).is_equal(0)
	assert_dict(p.held()).is_equal({"id": "bucket", "count": 1})
	assert_int(p.inv.count_of("water_bucket")).is_equal(1)
	p.select(1)
	assert_dict(p.held()).is_equal({"id": "water_bucket", "count": 1})
	p.replace_held_one("bucket")
	assert_dict(p.held()).is_equal({"id": "bucket", "count": 1})


# ---------- inventory screen ----------

func test_shift_click_moves_between_hotbar_and_main() -> void:
	p.inv.set_slot(2, {"id": "oak_log", "count": 10})
	p.click_slot(2, MOUSE_BUTTON_LEFT, true)
	assert_bool(p.inv.is_empty_slot(2)).is_true()
	assert_dict(p.inv.get_slot(9)).is_equal({"id": "oak_log", "count": 10})
	p.click_slot(9, MOUSE_BUTTON_LEFT, true)
	assert_dict(p.inv.get_slot(0)).is_equal({"id": "oak_log", "count": 10})


func test_cursor_left_right_click() -> void:
	p.inv.set_slot(0, {"id": "sand", "count": 9})
	p.click_slot(0, MOUSE_BUTTON_RIGHT, false)   # pick up half (5)
	assert_dict(p.cursor).is_equal({"id": "sand", "count": 5})
	p.click_slot(1, MOUSE_BUTTON_RIGHT, false)   # place one
	assert_dict(p.inv.get_slot(1)).is_equal({"id": "sand", "count": 1})
	p.click_slot(2, MOUSE_BUTTON_LEFT, false)    # place the rest
	assert_dict(p.cursor).is_empty()
	assert_dict(p.inv.get_slot(2)).is_equal({"id": "sand", "count": 4})


func test_personal_grid_crafts_planks_and_sticks() -> void:
	p.inv.set_slot(0, {"id": "oak_log", "count": 2})
	p.click_slot(0, MOUSE_BUTTON_LEFT, false)
	p.click_craft_slot(0, MOUSE_BUTTON_RIGHT, false)   # one log into the grid
	assert_dict(p.craft.preview_output()).is_equal({"id": "oak_planks", "count": 4})
	p.click_slot(0, MOUSE_BUTTON_LEFT, false)           # cursor (1 log) back to slot 0
	p.click_craft_output(false)
	assert_dict(p.cursor).is_equal({"id": "oak_planks", "count": 4})
	assert_bool(p.craft.grid.is_empty_slot(0)).is_true()
	# planks vertical in the 2x2 -> sticks via shift-click straight into the inventory
	p.click_craft_slot(0, MOUSE_BUTTON_RIGHT, false)
	p.click_craft_slot(2, MOUSE_BUTTON_RIGHT, false)
	p.click_slot(5, MOUSE_BUTTON_LEFT, false)           # park the remaining 2 planks
	assert_dict(p.craft.preview_output()).is_equal({"id": "stick", "count": 4})
	p.click_craft_output(true)
	assert_int(p.inv.count_of("stick")).is_equal(4)
	assert_dict(p.craft.preview_output()).is_empty()


func test_close_screen_returns_grid_and_cursor() -> void:
	p.inv.set_slot(0, {"id": "oak_log", "count": 3})
	p.click_slot(0, MOUSE_BUTTON_LEFT, false)
	p.click_craft_slot(1, MOUSE_BUTTON_RIGHT, false)
	var dropped := p.close_screen()
	assert_array(dropped).is_empty()
	assert_dict(p.cursor).is_empty()
	assert_int(p.inv.count_of("oak_log")).is_equal(3)


func test_crafting_table_grid_swap() -> void:
	p.inv.set_slot(0, {"id": "oak_planks", "count": 8})
	p.click_slot(0, MOUSE_BUTTON_LEFT, false)
	p.click_craft_slot(0, MOUSE_BUTTON_RIGHT, false)
	assert_array(p.set_crafting_grid(3, "crafting_table")).is_empty()
	assert_int(p.craft.width).is_equal(3)
	assert_int(p.inv.count_of("oak_planks")).is_equal(1)   # the grid's plank came back


# ---------- health ----------

func test_damage_and_respawn() -> void:
	p.damage(7)
	assert_int(p.health).is_equal(13)
	p.damage(100)
	assert_bool(p.is_dead()).is_true()
	p.respawn()
	assert_int(p.health).is_equal(20)
	p.heal(5)
	assert_int(p.health).is_equal(20)


func test_summary_is_serialisable() -> void:
	p.pickup("dirt", 3)
	var s := p.summary()
	assert_str(JSON.stringify(s)).contains("dirt")
	assert_int(s["health"]).is_equal(20)
