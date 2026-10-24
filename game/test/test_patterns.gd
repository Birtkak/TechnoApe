extends GdUnitTestSuite

## Patterned building materials (ADR 0039): which tiles the shader lays over the grid, the new
## cobblestone and bricks materials, and the way from clay to a bricks block.


func test_patterned_tiles_are_flagged_for_the_shader() -> void:
	var bits := GameMaterials.pattern_tiles()
	var on := func(t: int) -> bool: return (bits[t / 32] & (1 << (t % 32))) != 0
	for t in [39, 85, 73, 88, 89, 98, 99]:   # planks, oak and birch bark, thatch, cobblestone, bricks
		assert_bool(on.call(t)).override_failure_message("tile %d" % t).is_true()
	for t in [0, 1, 2, 20, 56]:              # stone, dirt, grass, leaves, log rings keep their palettes
		assert_bool(on.call(t)).override_failure_message("tile %d" % t).is_false()


func test_cobblestone_and_bricks_are_appended_materials() -> void:
	assert_str(GameMaterials.name_of(GameMaterials.COBBLESTONE)).is_equal("cobblestone")
	assert_str(GameMaterials.name_of(GameMaterials.BRICKS)).is_equal("bricks")
	assert_int(GameMaterials.COBBLESTONE).is_equal(GameMaterials.LEAF_LITTER + 1)
	assert_bool(GameMaterials.is_plain_voxel(GameMaterials.BRICKS)).is_true()


func test_clay_becomes_a_bricks_block() -> void:
	var items := ItemRegistry.load_default()
	var recipes := RecipeRegistry.load_default(items)
	assert_array(recipes.errors).is_empty()
	assert_str(items.get_def("cobblestone")["material"]).is_equal("cobblestone")
	assert_str(items.get_def("bricks")["material"]).is_equal("bricks")
	var chain := {"raw_clay_brick": "clayform", "clay_brick": "kiln", "bricks": "shapeless"}
	for id in chain:
		var types := []
		for rid in recipes.makes(id):
			types.append(recipes.recipes[rid]["type"])
		assert_array(types).override_failure_message(id).contains([chain[id]])
	var mining := MiningTable.load_default()
	assert_bool(mining.is_minable("bricks")).is_true()
	assert_bool(mining.is_minable("cobblestone")).is_true()
