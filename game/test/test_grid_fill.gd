extends GdUnitTestSuite

## The recipe browser's Fill button and the grid's Clear button (bug report 2026-10-25):
## CraftingGrid.fill lays a recipe out from the inventory, once or as many times as the
## inventory and the stack limits allow; all or nothing when ingredients are short.

var items: ItemRegistry
var recipes: RecipeRegistry
var inv: Inventory


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	inv = Inventory.new(items, PlayerInventory.SIZE)


func _r(id: String) -> Dictionary:
	return recipes.recipes[id]


func _u(id: String) -> int:
	return items.units(id)


func test_fill_once_with_enough_items() -> void:
	var g := CraftingGrid.new(items, recipes, 3, "crafting_table")
	inv.add("oak_planks", 9 * _u("oak_planks"))
	var res := g.fill(_r("chest"), [inv])
	assert_str(res["error"]).is_empty()
	assert_int(res["crafts"]).is_equal(1)
	assert_str(g.current_recipe().get("id", "")).is_equal("chest")
	assert_bool(g.grid.is_empty_slot(4)).is_true()   # the chest's hollow middle
	assert_int(int(g.grid.get_slot(0)["count"])).is_equal(_u("oak_planks"))
	assert_int(inv.count_of("oak_planks")).is_equal(_u("oak_planks"))   # one block's worth left over
	assert_str(g.craft_once().get("id", "")).is_equal("chest")


func test_shaped_recipe_in_the_personal_grid() -> void:
	var g := CraftingGrid.new(items, recipes, 2, "")
	inv.add("oak_planks", 2 * _u("oak_planks"))
	var res := g.fill(_r("sticks"), [inv])
	assert_int(res["crafts"]).is_equal(1)
	assert_bool(g.grid.is_empty_slot(0)).is_false()
	assert_bool(g.grid.is_empty_slot(2)).is_false()   # the column, row by row in the 2x2
	assert_str(g.current_recipe().get("id", "")).is_equal("sticks")


func test_missing_items_move_nothing() -> void:
	var g := CraftingGrid.new(items, recipes, 3, "crafting_table")
	inv.add("oak_planks", 3 * _u("oak_planks"))
	g.grid.set_slot(4, {"id": "cobblestone", "count": 5})
	var res := g.fill(_r("chest"), [inv], 1 << 20)
	assert_int(res["crafts"]).is_equal(0)
	assert_str(res["error"]).contains("Missing")
	assert_array(res["missing"]).has_size(1)
	assert_str(res["missing"][0]["id"]).is_equal("oak_planks")
	assert_int(res["missing"][0]["have"]).is_equal(3 * _u("oak_planks"))
	assert_int(res["missing"][0]["need"]).is_equal(8 * _u("oak_planks"))
	assert_int(inv.count_of("oak_planks")).is_equal(3 * _u("oak_planks"))
	assert_str(g.grid.get_slot(4)["id"]).is_equal("cobblestone")   # the grid is left alone too


func test_station_and_size_refusals() -> void:
	var small := CraftingGrid.new(items, recipes, 2, "")
	inv.add("oak_planks", 9 * _u("oak_planks"))
	assert_str(small.fill(_r("chest"), [inv])["error"]).contains("Needs")
	assert_str(small.cannot_hold(_r("firestarter"))).is_empty()   # three ingredients fit the 2x2
	var five := {"id": "t", "type": "shapeless", "ingredients": ["stick", "stick", "stick", "stick", "stick"], "output": {"id": "torch", "count": 1}}
	assert_str(small.cannot_hold(five)).contains("3x3")
	var wide := {"id": "t", "type": "shaped", "pattern": ["SSS"], "key": {"S": "stick"}, "output": {"id": "torch", "count": 1}}
	assert_str(small.cannot_hold(wide)).contains("3x3")
	assert_int(inv.count_of("oak_planks")).is_equal(9 * _u("oak_planks"))


func test_shift_fill_is_capped_by_the_inventory() -> void:
	# the bug report's case: 10 sticks and 21 dry grass make 5 firestarters
	var g := CraftingGrid.new(items, recipes, 3, "crafting_table")
	inv.add("stick", 10)
	inv.add("dry_grass", 21)
	var res := g.fill(_r("firestarter"), [inv], 1 << 20)
	assert_int(res["crafts"]).is_equal(5)
	assert_int(inv.count_of("stick")).is_equal(0)
	assert_int(inv.count_of("dry_grass")).is_equal(16)
	assert_int(int(g.grid.get_slot(0)["count"])).is_equal(5)
	assert_int(int(g.grid.get_slot(2)["count"])).is_equal(5)


func test_shift_fill_is_capped_by_the_stack_limit() -> void:
	var g := CraftingGrid.new(items, recipes, 3, "crafting_table")
	inv.add("stick", 200)
	inv.add("dry_grass", 200)
	var res := g.fill(_r("torch_grass"), [inv], 1 << 20)
	var cap := items.max_stack("stick") / _u("stick")
	assert_int(res["crafts"]).is_equal(cap)
	assert_int(int(g.grid.get_slot(0)["count"])).is_equal(cap)
	assert_int(inv.count_of("stick")).is_equal(200 - cap)
	# voxel items: a slot holds max_stack voxels, one block's worth per craft
	var g2 := CraftingGrid.new(items, recipes, 2, "")
	var inv2 := Inventory.new(items, PlayerInventory.SIZE)
	inv2.add("oak_log", 100 * _u("oak_log"))
	var res2 := g2.fill(_r("planks_from_log"), [inv2], 1 << 20)
	assert_int(res2["crafts"]).is_equal(items.max_stack("oak_log") / _u("oak_log"))
	assert_int(int(g2.grid.get_slot(0)["count"])).is_less_equal(items.max_stack("oak_log"))


func test_grid_contents_come_back_first_and_count() -> void:
	var g := CraftingGrid.new(items, recipes, 3, "crafting_table")
	g.grid.set_slot(4, {"id": "stick", "count": 3})
	g.grid.set_slot(8, {"id": "cobblestone", "count": 7})
	inv.add("dry_grass", 2)
	var res := g.fill(_r("torch_grass"), [inv], 1 << 20)
	assert_int(res["crafts"]).is_equal(2)   # the grid's own 3 sticks count; the grass limits it
	assert_int(inv.count_of("cobblestone")).is_equal(7)
	assert_int(inv.count_of("stick")).is_equal(1)
	assert_str(g.current_recipe().get("id", "")).is_equal("torch_grass")


func test_kept_tool_goes_in_once_with_its_wear() -> void:
	var saw: String = items.items_with_tag("saw")[0]
	var g := CraftingGrid.new(items, recipes, 2, "")
	inv.add("oak_log", 3 * _u("oak_log"))
	inv.add(saw, 1, {"wear": 5})
	var res := g.fill(_r("planks_with_saw"), [inv], 1 << 20)
	assert_int(res["crafts"]).is_equal(3)
	var s: Dictionary = g.grid.get_slot(1)
	assert_str(s["id"]).is_equal(saw)
	assert_int(int(s["count"])).is_equal(1)
	assert_int(int(s.get("wear", 0))).is_equal(5)
	assert_int(int(g.grid.get_slot(0)["count"])).is_equal(3 * _u("oak_log"))


func test_dry_run_moves_nothing() -> void:
	var g := CraftingGrid.new(items, recipes, 3, "crafting_table")
	inv.add("stick", 10)
	inv.add("dry_grass", 21)
	var res := g.fill(_r("firestarter"), [inv], 1 << 20, true)
	assert_int(res["crafts"]).is_equal(5)
	assert_int(inv.count_of("stick")).is_equal(10)
	assert_bool(g.grid.is_empty_slot(0)).is_true()


func test_clear_returns_the_grid() -> void:
	var p := PlayerInventory.new(items, recipes)
	p.set_crafting_grid(3, "crafting_table")
	p.craft.grid.set_slot(0, {"id": "stick", "count": 4})
	p.craft.grid.set_slot(5, {"id": "dry_grass", "count": 2})
	assert_bool(p.clear_craft_grid()).is_true()
	assert_int(p.inv.count_of("stick")).is_equal(4)
	assert_int(p.inv.count_of("dry_grass")).is_equal(2)
	for i in p.craft.grid.size():
		assert_bool(p.craft.grid.is_empty_slot(i)).is_true()


# ---------- Fill on a furnace screen (ADR 0093) ----------

func test_fill_furnace_input_from_a_smelt_recipe() -> void:
	var p := PlayerInventory.new(items, recipes)
	var f := FurnaceEntity.new(items, recipes, Vector3i.ZERO)
	p.open_container(f)
	p.inv.add("sand", 5 * _u("sand"))
	var r := _r("smelt_glass")   # "#sand": the member the inventory holds is picked
	assert_int(p.fill_container(r, true, true)["crafts"]).is_equal(5)
	assert_int(f.inv.count_of("sand")).is_equal(0)   # dry run moves nothing
	assert_str(p.fill_container(r, false)["error"]).is_empty()
	assert_int(f.inv.count_of("sand")).is_equal(_u("sand"))
	assert_int(p.fill_container(r, true)["crafts"]).is_equal(4)
	assert_int(f.inv.count_of("sand")).is_equal(5 * _u("sand"))
	assert_int(p.inv.count_of("sand")).is_equal(0)
	assert_str(p.fill_container(r, false)["error"]).is_equal("None in the inventory")


func test_fill_furnace_refuses_other_stations_and_a_full_slot() -> void:
	var p := PlayerInventory.new(items, recipes)
	var f := FurnaceEntity.new(items, recipes, Vector3i.ZERO)
	p.open_container(f)
	assert_str(p.fill_container(_r("cook_meat_firepit"), false)["error"]).is_not_empty()
	f.inv.set_slot(0, {"id": "cobblestone", "count": 4})
	p.inv.add("sand", _u("sand"))
	assert_str(p.fill_container(_r("smelt_glass"), false)["error"]).is_equal("The input slot holds Cobblestone")
	assert_int(p.inv.count_of("sand")).is_equal(_u("sand"))
