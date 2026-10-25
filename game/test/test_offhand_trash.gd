extends GdUnitTestSuite

## ADR 0055 (bug reports 2026-10-25): the offhand slot, the trash slot and drops from the
## inventory screen. Every move keeps per-stack data (spoil, wear, hot_until, filter).

var items: ItemRegistry
var recipes: RecipeRegistry
var p: PlayerInventory


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	p = PlayerInventory.new(items, recipes)


func test_swap_hands_keeps_stack_data() -> void:
	p.inv.set_slot(0, {"id": "flint_knife", "count": 1, "wear": 5})
	p.off.set_slot(0, {"id": "raw_meat", "count": 3, "spoil": 0.25})
	p.swap_hands()
	assert_str(p.held()["id"]).is_equal("raw_meat")
	assert_float(float(p.held()["spoil"])).is_equal_approx(0.25, 0.0001)
	assert_str(p.offhand()["id"]).is_equal("flint_knife")
	assert_int(int(p.offhand()["wear"])).is_equal(5)
	p.swap_hands()
	assert_str(p.held()["id"]).is_equal("flint_knife")
	assert_int(int(p.held()["wear"])).is_equal(5)


func test_swap_with_an_empty_hand() -> void:
	p.inv.set_slot(0, {"id": "torch", "count": 16})
	p.swap_hands()
	assert_bool(p.held().is_empty()).is_true()
	assert_int(int(p.offhand()["count"])).is_equal(16)


func test_offhand_slot_clicks_and_shift() -> void:
	p.inv.set_slot(3, {"id": "copper_hammer", "count": 1, "wear": 2})
	p.click_slot(3, MOUSE_BUTTON_LEFT, false)        # pick it up
	p.click_offhand(MOUSE_BUTTON_LEFT, false)        # into the offhand
	assert_bool(p.cursor.is_empty()).is_true()
	assert_int(int(p.offhand()["wear"])).is_equal(2)
	p.click_offhand(MOUSE_BUTTON_LEFT, true)         # shift: back into the inventory
	assert_bool(p.offhand().is_empty()).is_true()
	assert_int(p.inv.count_of("copper_hammer")).is_equal(1)


func test_offhand_consume_and_wear() -> void:
	p.off.set_slot(0, {"id": "clay_ball", "count": 5})
	assert_int(p.consume_offhand(2)).is_equal(2)
	assert_int(int(p.offhand()["count"])).is_equal(3)
	var dur := int(items.get_def("copper_hammer").get("durability", 0))
	assert_int(dur).is_greater(0)
	p.off.set_slot(0, {"id": "copper_hammer", "count": 1, "wear": dur - 1})
	assert_bool(p.wear_offhand(1)).is_true()    # the last use breaks it
	assert_bool(p.offhand().is_empty()).is_true()


func test_trash_keeps_the_last_stack_until_the_next() -> void:
	p.cursor = {"id": "raw_meat", "count": 2, "spoil": 0.5}
	p.click_trash()
	assert_bool(p.cursor.is_empty()).is_true()
	assert_float(float(p.trash["spoil"])).is_equal_approx(0.5, 0.0001)
	p.click_trash()                                  # empty cursor: it comes back
	assert_int(int(p.cursor["count"])).is_equal(2)
	assert_float(float(p.cursor["spoil"])).is_equal_approx(0.5, 0.0001)
	p.click_trash()
	p.cursor = {"id": "flint", "count": 1}
	p.click_trash()                                  # replaces the meat for good
	assert_str(p.trash["id"]).is_equal("flint")


func test_drop_cursor_all_or_one() -> void:
	p.cursor = {"id": "copper_ingot", "count": 4, "hot_until": Inventory.clock_s + 30.0}
	var one := p.drop_cursor(false)
	assert_int(int(one["count"])).is_equal(1)
	assert_bool(one.has("hot_until")).is_true()
	assert_int(int(p.cursor["count"])).is_equal(3)
	var rest := p.drop_cursor(true)
	assert_int(int(rest["count"])).is_equal(3)
	assert_bool(p.cursor.is_empty()).is_true()
	assert_bool(p.drop_cursor(true).is_empty()).is_true()


func test_drop_one_of_a_voxel_item_is_a_block() -> void:
	var u := items.units("dirt")
	p.cursor = {"id": "dirt", "count": u * 3}
	assert_int(int(p.drop_cursor(false)["count"])).is_equal(u)


func test_drop_from_a_slot_keeps_data() -> void:
	p.inv.set_slot(12, {"id": "flint_knife", "count": 1, "wear": 7})
	var s := p.drop_from("inv", 12, false)
	assert_int(int(s["wear"])).is_equal(7)
	assert_bool(p.inv.is_empty_slot(12)).is_true()
	p.inv.set_slot(13, {"id": "raw_meat", "count": 5, "spoil": 0.1})
	assert_int(int(p.drop_from("inv", 13, false)["count"])).is_equal(1)
	assert_int(int(p.drop_from("inv", 13, true)["count"])).is_equal(4)
	p.off.set_slot(0, {"id": "torch", "count": 3})
	assert_int(int(p.drop_from("offhand", 0, true)["count"])).is_equal(3)
	assert_bool(p.drop_from("inv", 20, true).is_empty()).is_true()   # an empty slot drops nothing


func test_mouse_tweaks_reach_the_offhand() -> void:
	p.inv.set_slot(0, {"id": "torch", "count": 10})
	p.off.set_slot(0, {"id": "torch", "count": 4})
	p.wheel_move("offhand", 0, false)                # wheel up pulls one in
	assert_int(int(p.offhand()["count"])).is_equal(5)


func test_vacuum_filter_survives_moves() -> void:
	p.inv.set_slot(0, {"id": "world_vacuum", "count": 1, "filter": ["sand", "gravel"]})
	p.swap_hands()
	assert_array(p.offhand()["filter"]).contains_exactly(["sand", "gravel"])
	var ids := PlaytestTools.filter_ids(p.offhand())
	assert_bool(ids.has(GameMaterials.SAND)).is_true()
	assert_bool(ids.has(GameMaterials.SAND_STATIC)).is_true()   # the static variant goes with it
	assert_bool(ids.has(GameMaterials.STONE)).is_false()


func test_kits_count_blocks_of_voxel_items() -> void:
	var reg := CommandRegistry.new()
	var _cmds := GameCommands.new(reg, null, p)   # kept alive: the registry holds its callables
	assert_bool(reg.execute("/kit builder")["ok"]).is_true()
	assert_int(p.inv.count_of("stone")).is_equal(64 * items.units("stone"))   # 64 blocks, not 64 voxels
