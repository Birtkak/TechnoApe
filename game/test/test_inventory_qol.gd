extends GdUnitTestSuite

## Plan items D11 and D12: auto-refill, the worn-tool warning, favourite slots, sorting, quick-stack,
## key strings for remapping, and graves keeping per-stack data.

var items: ItemRegistry
var recipes: RecipeRegistry
var p: PlayerInventory


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	p = PlayerInventory.new(items, recipes)


func test_refill_when_the_hand_runs_out() -> void:
	p.inv.set_slot(0, {"id": "torch", "count": 1})
	p.inv.set_slot(20, {"id": "torch", "count": 10})
	p.consume_held(1)
	assert_int(int(p.held().get("count", 0))).is_equal(10)
	assert_bool(p.inv.is_empty_slot(20)).is_true()


func test_refill_after_a_tool_breaks_and_low_warning() -> void:
	var dur := int(items.get_def("copper_axe")["durability"])
	var low := []
	p.tool_low.connect(func(id: String, left: int) -> void: low.append(left))
	p.inv.set_slot(0, {"id": "copper_axe", "count": 1, "wear": dur - maxi(3, dur / 10) - 1})
	p.inv.set_slot(30, {"id": "copper_axe", "count": 1, "wear": 2})
	p.wear_held(1)                      # crosses into the last tenth: one warning
	p.wear_held(1)
	assert_int(low.size()).is_equal(1)
	p.wear_held(dur)                    # breaks
	assert_int(int(p.held().get("wear", -1))).is_equal(2)   # the other axe, its wear kept
	assert_bool(p.inv.is_empty_slot(30)).is_true()


func test_favourites_stay_put() -> void:
	p.inv.set_slot(0, {"id": "torch", "count": 1})
	p.inv.set_slot(9, {"id": "torch", "count": 5})
	p.toggle_favourite(9)
	p.consume_held(1)
	assert_bool(p.held().is_empty()).is_true()   # no refill from a favourite
	p.click_slot(9, MOUSE_BUTTON_LEFT, true)     # shift-click does nothing
	assert_int(int(p.inv.get_slot(9)["count"])).is_equal(5)
	p.toggle_favourite(9)
	assert_bool(p.favourites.has(9)).is_false()


func test_sort_merges_orders_and_keeps_data() -> void:
	p.inv.set_slot(12, {"id": "raw_meat", "count": 2, "spoil": 0.5})
	p.inv.set_slot(20, {"id": "flint", "count": 3})
	p.inv.set_slot(25, {"id": "raw_meat", "count": 2, "spoil": 0.1})
	p.inv.set_slot(30, {"id": "flint_knife", "count": 1, "wear": 4})
	p.inv.set_slot(31, {"id": "flint_knife", "count": 1, "wear": 9})
	p.inv.set_slot(15, {"id": "stick", "count": 7})
	p.toggle_favourite(15)
	p.sort_inventory()
	assert_int(int(p.inv.get_slot(15)["count"])).is_equal(7)   # the favourite stayed
	var order := []
	for i in range(PlayerInventory.HOTBAR, PlayerInventory.SIZE):
		if i != 15 and not p.inv.is_empty_slot(i):
			order.append(p.inv.get_slot(i)["id"])
	assert_array(order).contains_exactly(["flint", "flint_knife", "flint_knife", "raw_meat"])
	var meat := {}
	for s in p.inv.slots:
		if s.get("id", "") == "raw_meat":
			meat = s
	assert_int(int(meat["count"])).is_equal(4)
	assert_float(float(meat["spoil"])).is_equal_approx(0.3, 0.001)   # count-weighted
	var wears := []
	for s in p.inv.slots:
		if s.get("id", "") == "flint_knife":
			wears.append(int(s["wear"]))
	assert_array(wears).contains_exactly_in_any_order([4, 9])


func test_quick_stack_only_what_a_chest_holds() -> void:
	var chest := ChestEntity.new(items, Vector3i.ZERO)
	chest.inv.set_slot(0, {"id": "flint", "count": 1})
	p.inv.set_slot(10, {"id": "flint", "count": 6})
	p.inv.set_slot(11, {"id": "stick", "count": 6})
	p.inv.set_slot(12, {"id": "flint", "count": 2})
	p.toggle_favourite(12)
	p.inv.set_slot(2, {"id": "flint", "count": 3})   # the hotbar stays
	var moved := p.quick_stack([chest])
	assert_int(moved).is_equal(6)
	assert_int(chest.inv.count_of("flint")).is_equal(7)
	assert_int(p.inv.count_of("flint")).is_equal(5)
	assert_int(p.inv.count_of("stick")).is_equal(6)


func test_chest_label_saves() -> void:
	var chest := ChestEntity.new(items, Vector3i(1, 2, 3))
	chest.label = "Ores"
	var c2 := ChestEntity.new(items, Vector3i(1, 2, 3))
	c2.load_from(chest.to_dict())
	assert_str(c2.label).is_equal("Ores")


func test_key_strings_round_trip() -> void:
	var k := InputEventKey.new()
	k.physical_keycode = KEY_G
	var back := Settings.key_event(Settings.key_string(k))
	assert_int((back as InputEventKey).physical_keycode).is_equal(KEY_G)
	var m := InputEventMouseButton.new()
	m.button_index = MOUSE_BUTTON_XBUTTON1
	assert_int((Settings.key_event(Settings.key_string(m)) as InputEventMouseButton).button_index).is_equal(MOUSE_BUTTON_XBUTTON1)
	assert_object(Settings.key_event("nonsense")).is_null()


func test_rebind_and_reset() -> void:
	var g := InputEventKey.new()
	g.physical_keycode = KEY_G
	Settings.bind_key("map", g)
	assert_str(Settings.key_text(Settings.current_key("map"))).is_equal(Settings.key_text(g))
	Settings.reset_keys()
	assert_int((Settings.current_key("map") as InputEventKey).physical_keycode).is_equal(KEY_M)
	assert_int((Settings.current_key("sprint") as InputEventKey).physical_keycode).is_equal(KEY_CTRL)


func test_grave_keeps_stack_data() -> void:
	var g := GraveEntity.new(items, Vector3i(8, 1040, 8))
	g.set_stacks([{"id": "flint_knife", "count": 1, "wear": 3}, {}, {"id": "raw_meat", "count": 2, "spoil": 0.4}])
	assert_int(g.count()).is_equal(2)
	var g2 := GraveEntity.new(items, g.pos)
	var saved: Dictionary = JSON.parse_string(JSON.stringify(g.to_dict()))
	g2.load_from(saved)
	var got := g2.take_all()
	assert_int(int(got[0]["wear"])).is_equal(3)
	assert_float(float(got[1]["spoil"])).is_equal_approx(0.4, 0.0001)
	assert_int(g2.count()).is_equal(0)
	assert_object(g.view_parts().get("solid")).is_not_null()
