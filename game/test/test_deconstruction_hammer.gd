extends GdUnitTestSuite

## The deconstruction hammer: built materials come apart at once and drop themselves, natural ground
## stays, and it works a full 1 m brush.


func test_hammer_breaks_built_blocks_at_once_and_leaves_the_ground() -> void:
	var mining := MiningTable.load_default()
	var hammer: Dictionary = ItemRegistry.load_default().get_def("deconstruction_hammer")
	assert_bool(MiningTable.deconstructs(hammer)).is_true()
	assert_float(mining.break_time("bricks", hammer)).is_equal(0.0)
	assert_float(mining.break_time("planks", hammer)).is_equal(0.0)
	assert_float(mining.break_time("stone", hammer)).is_equal(-1.0)
	assert_float(mining.break_time("dirt", hammer)).is_equal(-1.0)
	assert_bool(mining.yields("bricks", hammer)).is_true()
	assert_str(mining.drops("bricks", hammer).get("id", "")).is_equal("bricks")
	assert_int(MiningTable.dig_cap(hammer)).is_equal(8)
	# a plain tool keeps its old answers
	assert_float(mining.break_time("stone", {})).is_greater(0.0)
