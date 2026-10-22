extends GdUnitTestSuite

## Headless tests for the JEI-style recipe browser model (ADR 0008).

var items: ItemRegistry
var recipes: RecipeRegistry
var b: RecipeBrowser


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	b = RecipeBrowser.new(items, recipes)


func test_list_shows_every_item_in_file_order() -> void:
	assert_int(b.filtered().size()).is_equal(items.all_ids().size())
	assert_str(b.filtered()[0]).is_equal(items.all_ids()[0])


func test_search_terms_and_tags() -> void:
	b.set_query("ingot")
	assert_array(b.filtered()).contains(["iron_ingot", "copper_ingot", "tin_ingot", "bronze_ingot"])
	b.set_query("IRON ingot")
	assert_array(b.filtered()).contains_exactly(["iron_ingot"])
	b.set_query("#metal")
	assert_array(b.filtered()).contains_exactly_in_any_order(["iron_ingot", "copper_ingot", "bronze_ingot", "tin_ingot"])
	b.set_query("#fuel")
	assert_array(b.filtered()).contains(["coal", "charcoal"])
	b.set_query("zzz")
	assert_array(b.filtered()).is_empty()
	assert_int(b.page_count()).is_equal(1)


func test_paging_wraps() -> void:
	b.page_size = 10
	var n := b.page_count()
	assert_int(n).is_equal(ceili(items.all_ids().size() / 10.0))
	assert_int(b.page_items().size()).is_equal(10)
	b.turn_page(-1)
	assert_int(b.page).is_equal(n - 1)
	b.turn_page(1)
	assert_int(b.page).is_equal(0)


func test_recipes_for_ingot_is_smelting() -> void:
	assert_bool(b.show("iron_ingot", "recipes")).is_true()
	assert_str(b.current_category()["id"]).is_equal("smelting")
	assert_str(b.current_recipe()["id"]).is_equal("smelt_iron")
	var l := b.layout(b.current_recipe())
	assert_str(l["kind"]).is_equal("smelt")
	assert_str(l["info"]).is_equal("10.0 s")
	assert_str(l["needs"]).is_equal("")


func test_uses_of_planks_and_navigation() -> void:
	assert_bool(b.show("oak_planks", "uses")).is_true()
	var ids: Array = []
	for c in b.view["categories"]:
		ids.append_array(c["recipes"])
	assert_array(ids).contains(["sticks", "crafting_table", "chest", "wooden_pickaxe"])
	assert_str(b.current_category()["id"]).is_equal("crafting")
	b.turn_recipe(-1)
	assert_int(b.view["index"]).is_equal(b.current_category()["recipes"].size() - 1)
	# click an ingredient: recipes of sticks, then back
	assert_bool(b.show("stick", "recipes")).is_true()
	assert_int(b.history.size()).is_equal(1)
	b.back()
	assert_str(b.view["item"]).is_equal("oak_planks")
	assert_int(b.view["index"]).is_equal(b.current_category()["recipes"].size() - 1)   # kept its place
	b.back()
	assert_bool(b.is_open()).is_false()


func test_nothing_to_show_changes_nothing() -> void:
	assert_bool(b.show("wand_basic", "recipes")).is_false()   # wands are found, never crafted
	assert_bool(b.is_open()).is_false()


func test_station_uses_include_its_category() -> void:
	assert_bool(b.show("furnace", "uses")).is_true()
	var cats: Array = b.view["categories"].map(func(c: Dictionary) -> String: return c["id"])
	assert_array(cats).contains(["smelting"])
	var smelting: Dictionary = b.view["categories"][cats.find("smelting")]
	assert_int(smelting["recipes"].size()).is_equal(recipes.recipes.values().filter(func(r: Dictionary) -> bool: return r["type"] == "smelt").size())
	assert_bool(b.show("crafting_table", "uses")).is_true()
	assert_str(b.current_category()["id"]).is_equal("crafting")
	assert_array(b.catalysts("crafting")).contains_exactly(["crafting_table"])


func test_layouts_place_inputs() -> void:
	var sticks := b.layout(recipes.recipes["sticks"])
	assert_str(sticks["kind"]).is_equal("grid")
	assert_array(sticks["inputs"].map(func(i: Dictionary) -> Vector2i: return i["cell"])).contains_exactly([Vector2i(0, 0), Vector2i(0, 1)])
	var pick := b.layout(recipes.recipes["wooden_pickaxe"])
	assert_int(pick["inputs"].size()).is_equal(5)
	assert_str(pick["needs"]).is_equal("Crafting Table")
	var bronze := b.layout(recipes.recipes["bronze_alloy"])
	assert_str(bronze["info"]).is_equal("Shapeless")
	assert_str(b.category_of(recipes.recipes["bronze_alloy"])).is_equal("station:furnace")
	assert_str(b.category_title("station:furnace")).is_equal("Furnace")


func test_every_recipe_lays_out_and_every_ingredient_resolves() -> void:
	for rid in recipes.recipes:
		var l := b.layout(recipes.recipes[rid])
		assert_int(l["inputs"].size()).override_failure_message("%s has no inputs" % rid).is_greater(0)
		for inp in l["inputs"]:
			assert_str(b.display_item(inp["spec"], 0)).override_failure_message("%s: %s resolves to nothing" % [rid, inp["spec"]]).is_not_empty()


func test_tag_ingredients_cycle() -> void:
	var m := b.members("#metal.iron")
	assert_array(m).contains(["iron_ingot"])
	var logs := b.members("#log")
	for t in logs.size() * 2:
		assert_str(b.display_item("#log", t)).is_equal(logs[t % logs.size()])
	assert_str(b.display_item("stick", 5)).is_equal("stick")


func test_summary_is_serialisable() -> void:
	b.show("iron_ingot", "recipes")
	var s := b.summary()
	assert_str(s["view"]["recipe"]).is_equal("smelt_iron")
	assert_str(JSON.stringify(s)).is_not_empty()
