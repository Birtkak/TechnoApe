extends GdUnitTestSuite

## ADR 0168: an LV machine's screen mirrors its inputs and output into one container inventory and writes
## every change back to the machine; the output slot takes nothing in, ingredients shift to the inputs.

var items: ItemRegistry
var recipes: RecipeRegistry


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)


func test_slots_write_back_to_the_machine() -> void:
	var m := MachineEntity.new(items, recipes, Vector3i.ZERO, "lathe")
	var sc := MachineScreen.new(m)
	assert_int(sc.inv.size()).is_equal(m.inv.size() + 1)
	assert_bool(sc.may_place(0, "iron_ingot")).is_true()
	assert_bool(sc.may_place(0, "oak_log")).is_false()           # the lathe takes no logs
	assert_bool(sc.may_place(sc.inputs(), "iron_ingot")).is_false()   # nothing goes into the output
	assert_bool(sc.is_output(sc.inputs())).is_true()
	assert_array(sc.shift_targets("iron_ingot")).is_equal([0])
	assert_array(sc.shift_targets("oak_log")).is_empty()
	# the player puts four ingots in: the machine has them and finds its recipe
	var cursor := sc.inv.left_click(0, Inventory.make_stack(items, "iron_ingot", 4))
	assert_bool(cursor.is_empty()).is_true()
	assert_int(int(m.inv.get_slot(0).get("count", 0))).is_equal(4)
	assert_bool(m.current().is_empty()).is_false()
	assert_str(sc.status()).contains("no power")
	# the machine finishes: the screen shows the result, taking it empties the machine's output
	m.output = {"id": "iron_rod", "count": 2}
	m.changed.emit()
	assert_str(String(sc.inv.get_slot(sc.inputs()).get("id", ""))).is_equal("iron_rod")
	var got := sc.inv.take(sc.inputs())
	assert_int(int(got.get("count", 0))).is_equal(2)
	assert_bool(m.output.is_empty()).is_true()
	# a hopper takes an ingot out of the machine itself: the screen follows
	m.inv.take(0, 1)
	m.changed.emit()
	assert_int(int(sc.inv.get_slot(0).get("count", 0))).is_equal(3)
	sc.detach()
	assert_bool(m.changed.is_connected(sc.refresh)).is_false()


func test_every_machine_lays_out_its_slots() -> void:
	for kind in MachineEntity.DOOR:
		if kind == "builder":
			continue
		var m := MachineEntity.new(items, recipes, Vector3i.ZERO, kind)
		var pos: Array = MachineScreen.new(m).slot_positions()
		assert_int(pos.size()).is_equal(m.inv.size() + 1)
		for i in pos.size():
			for k in i:
				assert_bool(pos[i] == pos[k]).override_failure_message("%s: slots %d and %d overlap" % [kind, i, k]).is_false()
			assert_bool(pos[i].x >= 0 and pos[i].x + 32 <= 296).override_failure_message("%s slot %d at %s" % [kind, i, pos[i]]).is_true()
