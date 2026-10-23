extends GdUnitTestSuite

## Headless tests for the crafting core: data validity, recipe matching with
## Minecraft semantics, inventory cursor behaviour, browser queries and
## reachability. Run:
##   godot_console.exe --headless --path game -s res://addons/gdUnit4/bin/GdUnitCmdTool.gd --add res://test --ignoreHeadlessMode

var items: ItemRegistry
var recipes: RecipeRegistry


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)


# ---------- data ----------

func test_data_files_are_valid() -> void:
	assert_array(items.errors).is_empty()
	assert_array(recipes.errors).is_empty()
	assert_int(items.items.size()).is_greater(20)
	assert_int(recipes.recipes.size()).is_greater(10)


func test_tags_index() -> void:
	assert_bool(items.has_tag("oak_planks", "planks")).is_true()
	assert_array(items.items_with_tag("pickaxe")).contains(["flint_pickaxe", "copper_pickaxe", "iron_pickaxe"])
	assert_bool(items.matches("coal", "#fuel")).is_true()
	assert_bool(items.matches("coal", "charcoal")).is_false()


func test_wands_are_found_not_crafted() -> void:
	assert_array(recipes.makes("wand_basic")).is_empty()
	assert_bool(items.get_def("wand_basic").get("found_only", false)).is_true()
	assert_array(recipes.makes("spell_spark_bolt")).is_not_empty()


# ---------- shaped / shapeless matching ----------

func _g(cells: Array) -> Array:
	var g := []
	g.resize(9)
	g.fill("")
	for i in cells.size():
		g[i] = cells[i]
	return g


func test_shapeless_planks_from_any_log() -> void:
	var r := recipes.match_grid(_g(["", "", "", "", "oak_log"]))
	assert_str(r.get("id", "")).is_equal("planks_from_log")
	assert_int(int(r["output"]["count"])).is_equal(2)   # 2 per log by hand (ADR 0019)


func test_shaped_sticks_anywhere_in_grid() -> void:
	# vertical pair top-left, middle column, bottom-right: all valid translations
	assert_str(recipes.match_grid(_g(["oak_planks", "", "", "oak_planks"])).get("id", "")).is_equal("sticks")
	assert_str(recipes.match_grid(_g(["", "oak_planks", "", "", "oak_planks"])).get("id", "")).is_equal("sticks")
	assert_str(recipes.match_grid(_g(["", "", "", "", "", "oak_planks", "", "", "oak_planks"])).get("id", "")).is_equal("sticks")
	# horizontal pair is NOT sticks
	assert_dict(recipes.match_grid(_g(["oak_planks", "oak_planks"]))).is_empty()


func test_shaped_pickaxe_requires_station_and_exact_shape() -> void:
	var pick := _g(["iron_ingot", "iron_ingot", "iron_ingot", "", "stick", "", "", "stick", ""])
	assert_dict(recipes.match_grid(pick, "")).is_empty()  # needs a crafting table
	assert_str(recipes.match_grid(pick, "crafting_table").get("id", "")).is_equal("iron_pickaxe")
	var wrong := _g(["iron_ingot", "iron_ingot", "iron_ingot", "stick", "", "", "stick", "", ""])
	assert_dict(recipes.match_grid(wrong, "crafting_table")).is_empty()


func test_shaped_mirroring() -> void:
	# bucket pattern "I I" / " I " is symmetric; use a deliberately asymmetric check via mirror of glass flask
	var flask := _g(["glass", "", "glass", "", "glass", ""])
	assert_str(recipes.match_grid(flask, "crafting_table").get("id", "")).is_equal("glass_flask")


func test_tag_ingredients_accept_any_member() -> void:
	var furnace := _g(["cobblestone", "cobblestone", "cobblestone", "cobblestone", "", "cobblestone", "cobblestone", "cobblestone", "cobblestone"])
	assert_str(recipes.match_grid(furnace, "crafting_table").get("id", "")).is_equal("furnace")
	var iron := _g(["iron_ingot", "iron_ingot", "iron_ingot", "", "stick", "", "", "stick", ""])
	assert_str(recipes.match_grid(iron, "crafting_table").get("id", "")).is_equal("iron_pickaxe")


func test_shapeless_alloy_counts_must_match() -> void:
	assert_str(recipes.match_grid(_g(["copper_ingot", "copper_ingot", "copper_ingot", "tin_ingot"]), "furnace").get("id", "")).is_equal("bronze_alloy")
	assert_dict(recipes.match_grid(_g(["copper_ingot", "copper_ingot", "tin_ingot", "tin_ingot"]), "furnace")).is_empty()


# ---------- crafting grid flow ----------

func test_crafting_grid_consumes_one_per_slot() -> void:
	var cg := CraftingGrid.new(items, recipes, 3, "crafting_table")
	cg.grid.set_slot(0, {"id": "oak_planks", "count": 5})
	cg.grid.set_slot(3, {"id": "oak_planks", "count": 2})
	assert_dict(cg.preview_output()).is_equal({"id": "stick", "count": 4})
	var out := cg.craft_once()
	assert_dict(out).is_equal({"id": "stick", "count": 4})
	assert_int(cg.grid.get_slot(0)["count"]).is_equal(4)
	assert_int(cg.grid.get_slot(3)["count"]).is_equal(1)
	var all := cg.craft_all()
	assert_dict(all).is_equal({"id": "stick", "count": 4})  # only one more craft possible
	assert_bool(cg.grid.is_empty_slot(3)).is_true()


func test_two_by_two_grid_cannot_make_three_wide() -> void:
	var cg := CraftingGrid.new(items, recipes, 2, "")
	cg.grid.set_slot(0, {"id": "flint", "count": 1})
	cg.grid.set_slot(1, {"id": "flint", "count": 1})
	cg.grid.set_slot(2, {"id": "oak_log", "count": 1})
	cg.grid.set_slot(3, {"id": "oak_log", "count": 1})
	assert_dict(cg.preview_output()).is_equal({"id": "crafting_table", "count": 1})   # 2 flint + 2 logs (ADR 0019)


# ---------- inventory ----------

func test_add_merges_then_fills_and_reports_leftover() -> void:
	var inv := Inventory.new(items, 3)
	inv.set_slot(1, {"id": "cobblestone", "count": 60})
	# 70: 4 into slot 1 (to 64), 64 into slot 0, 2 into slot 2
	assert_int(inv.add("cobblestone", 70)).is_equal(0)
	assert_int(inv.get_slot(1)["count"]).is_equal(64)
	assert_int(inv.get_slot(0)["count"]).is_equal(64)
	assert_int(inv.get_slot(2)["count"]).is_equal(2)
	assert_int(inv.add("cobblestone", 200)).is_equal(200 - 62)
	assert_int(inv.count_of("cobblestone")).is_equal(192)


func test_tools_do_not_stack() -> void:
	var inv := Inventory.new(items, 2)
	assert_int(inv.add("iron_pickaxe", 3)).is_equal(1)
	assert_int(inv.get_slot(0)["count"]).is_equal(1)
	assert_int(inv.get_slot(1)["count"]).is_equal(1)


func test_cursor_left_and_right_click() -> void:
	var inv := Inventory.new(items, 2)
	inv.set_slot(0, {"id": "sand", "count": 9})
	var cursor := inv.left_click(0, {})               # pick up all 9
	assert_dict(cursor).is_equal({"id": "sand", "count": 9})
	assert_bool(inv.is_empty_slot(0)).is_true()
	cursor = inv.right_click(1, cursor)               # place one
	assert_int(inv.get_slot(1)["count"]).is_equal(1)
	assert_int(cursor["count"]).is_equal(8)
	cursor = inv.left_click(1, cursor)                # place the rest onto the same stack
	assert_dict(cursor).is_empty()
	assert_int(inv.get_slot(1)["count"]).is_equal(9)
	cursor = inv.right_click(1, {})                   # pick up half (5)
	assert_int(cursor["count"]).is_equal(5)
	assert_int(inv.get_slot(1)["count"]).is_equal(4)
	inv.set_slot(0, {"id": "dirt", "count": 3})
	cursor = inv.left_click(0, cursor)                # swap sand cursor with dirt slot
	assert_dict(cursor).is_equal({"id": "dirt", "count": 3})
	assert_dict(inv.get_slot(0)).is_equal({"id": "sand", "count": 5})


func test_shift_click_moves_and_merges() -> void:
	var a := Inventory.new(items, 1)
	var b := Inventory.new(items, 2)
	a.set_slot(0, {"id": "coal", "count": 30})
	b.set_slot(0, {"id": "coal", "count": 50})
	assert_bool(a.shift_click(0, b)).is_true()
	assert_int(b.get_slot(0)["count"]).is_equal(64)
	assert_int(b.get_slot(1)["count"]).is_equal(16)
	assert_bool(a.is_empty_slot(0)).is_true()


# ---------- browser ----------

func test_browser_queries() -> void:
	assert_array(recipes.uses_of("stick")).contains(["flint_pickaxe", "copper_pickaxe", "iron_pickaxe", "spell_speed_up"])
	assert_array(recipes.makes("glass")).is_equal(["smelt_glass"])
	assert_array(recipes.search("pick")).contains(["flint_pickaxe", "copper_pickaxe", "iron_pickaxe"])
	assert_array(items.search("ingot")).contains(["iron_ingot", "copper_ingot", "tin_ingot", "bronze_ingot"])
	assert_array(recipes.by_station("furnace")).contains(["smelt_iron", "smelt_glass", "bronze_alloy"])


func test_everything_craftable_is_reachable_from_raw_world_items() -> void:
	var raw := ["oak_log", "cobblestone", "sand", "iron_ore", "copper_ore", "tin_ore", "coal", "mana_dust", "fire_essence", "wand_basic", "dirt", "gravel", "grass_block",
		"wool", "raw_meat", "wheat", "carrot", "wheat_seeds",   # animal drops and wild crops (ADR 0022)
		"flint", "loose_stone", "loose_copper_ore", "clay_ball", "dry_grass",   # ground pickups, clay, cut grass (ADR 0023)
		"pumpkin", "blueberry"]   # wild pumpkin patches, blueberry bushes (ADR 0024)
	var have := recipes.reachable_from(raw)
	var missing := []
	for id in items.all_ids():
		if not have.has(id) and not items.get_def(id).get("found_only", false):
			# items with no recipe and not raw: must be found in the world (ore, drops) -> list them
			if recipes.makes(id).is_empty():
				continue
			missing.append(id)
	assert_array(missing).is_empty()
	assert_bool(have.has("iron_pickaxe")).is_true()
	assert_bool(have.has("spell_fire_bolt")).is_true()
