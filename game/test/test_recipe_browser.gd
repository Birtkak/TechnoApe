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
	assert_array(b.filtered()).contains_exactly_in_any_order(["iron_ingot", "copper_ingot", "bronze_ingot", "tin_ingot",
		"iron_plate", "iron_rod", "bronze_pipe", "copper_pipe", "steel_ingot", "steel_plate", "copper_wire", "iron_gear", "copper_coil"])   # the steam age's metal parts (ADR 0097, 0099)
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


func test_recipes_for_glass_is_smelting() -> void:
	assert_bool(b.show("glass", "recipes")).is_true()
	assert_str(b.current_category()["id"]).is_equal("smelting")
	assert_str(b.current_recipe()["id"]).is_equal("smelt_glass")
	var l := b.layout(b.current_recipe())
	assert_str(l["kind"]).is_equal("smelt")
	assert_str(l["info"]).is_equal("10.0 s")
	assert_str(l["needs"]).is_equal("")
	# a bloomery takes its ore in batches (ADR 0070)
	var bl := b.layout(b.recipe("bloom_iron_nugget"))
	assert_str(bl["info"]).is_equal("8 x Iron Nugget, 120.0 s")
	assert_str(bl["needs"]).is_equal("Bloomery")


func test_uses_of_planks_and_navigation() -> void:
	assert_bool(b.show("oak_planks", "uses")).is_true()
	var ids: Array = []
	for c in b.view["categories"]:
		ids.append_array(c["recipes"])
	assert_array(ids).contains(["sticks", "chest", "sieve", "bed"])
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
	assert_int(smelting["recipes"].size()).is_equal(recipes.recipes.values().filter(func(r: Dictionary) -> bool: return r["type"] == "smelt" and r.get("station", "furnace") == "furnace").size())   # firepit and forge have their own tabs (ADR 0093)
	assert_bool(b.show("crafting_table", "uses")).is_true()
	assert_str(b.current_category()["id"]).is_equal("crafting")
	assert_array(b.catalysts("crafting")).contains_exactly(["crafting_table"])


func test_layouts_place_inputs() -> void:
	var sticks := b.layout(recipes.recipes["sticks"])
	assert_str(sticks["kind"]).is_equal("grid")
	assert_array(sticks["inputs"].map(func(i: Dictionary) -> Vector2i: return i["cell"])).contains_exactly([Vector2i(0, 0), Vector2i(0, 1)])
	var anvil := b.layout(recipes.recipes["bronze_anvil"])
	assert_int(anvil["inputs"].size()).is_equal(7)
	assert_str(anvil["needs"]).is_equal("Crafting Table")
	var bronze := b.layout(recipes.recipes["alloy_bronze_tin_nugget"])
	assert_str(bronze["info"]).is_equal("2 x Tin Nugget, 15 s")
	assert_str(b.category_of(recipes.recipes["alloy_bronze_tin_nugget"])).is_equal("station:firepit")
	assert_str(b.category_title("station:firepit")).is_equal("Firepit")


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
	b.show("glass", "recipes")
	var s := b.summary()
	assert_str(s["view"]["recipe"]).is_equal("smelt_glass")
	assert_str(JSON.stringify(s)).is_not_empty()


# ---------- mining pseudo-recipes ----------

func _mining(json: String) -> MiningTable:
	var t := MiningTable.new()
	t.load_json(json)
	return t


func test_mining_recipes_show_drops_and_tools() -> void:
	b.set_mining(_mining('{"materials": {'
		+ '"stone": {"hardness": 1.5, "tool": "pickaxe", "requires": true, "drops": "cobblestone"},'
		+ '"iron_ore": {"hardness": 3, "tool": "pickaxe", "requires": true, "drops": "iron_ore", "min_tier": 1},'
		+ '"dirt": {"hardness": 0.5, "tool": "shovel", "drops": "dirt"},'
		+ '"sand": {"hardness": 0.5, "tool": "shovel", "drops": "gravel"},'
		+ '"sand_static": {"hardness": 0.5, "tool": "shovel", "drops": "gravel"},'
		+ '"gravel": {"hardness": 0.6, "tool": "shovel", "drops": [{"id": "flint", "count": [1, 2], "chance": 0.25}]}}}'))
	var cats := b.recipes_for("cobblestone")
	assert_array(cats.map(func(c: Dictionary) -> String: return c["id"])).contains(["mining"])
	var r := b.recipe("mining:stone:cobblestone")
	assert_str(r["input"]).is_equal("stone")
	var l := b.layout(r)
	assert_str(l["kind"]).is_equal("mine")
	assert_str(l["needs"]).is_equal("Any pickaxe")
	# a self-drop needing a tool tier stays (it tells you which pick), plain dirt -> dirt does not
	assert_str(b.layout(b.recipe("mining:iron_ore:iron_ore"))["needs"]).is_equal("Copper Pickaxe or better")
	assert_dict(b.recipe("mining:dirt:dirt")).is_empty()
	# sand and sand_static collapse to one entry; uses of the block list it
	assert_int(b.recipes_for("gravel").filter(func(c: Dictionary) -> bool: return c["id"] == "mining")[0]["recipes"].size()).is_equal(1)
	assert_array(b.uses_for("sand").map(func(c: Dictionary) -> String: return c["id"])).contains(["mining"])


func test_mining_counts_and_chance_in_info() -> void:
	if not items.has("flint"):
		return
	b.set_mining(_mining('{"materials": {"gravel": {"hardness": 0.6, "tool": "shovel", "drops": [{"id": "flint", "count": [1, 2], "chance": 0.25}]}}}'))
	var l := b.layout(b.recipe("mining:gravel:flint"))
	assert_str(l["info"]).contains("1-2").contains("25")
	assert_str(l["needs"]).is_equal("Hand (faster with a shovel)")


func test_real_mining_table_loads_into_the_browser() -> void:
	b.set_mining(MiningTable.load_default())
	b.show("cobblestone", "recipes")
	assert_array(b.summary()["view"]["categories"]).contains(["mining"])


# ---------- bookmarks ----------

func test_bookmarks_toggle_filter_and_persist() -> void:
	assert_bool(b.toggle_bookmark("iron_ingot")).is_true()
	assert_bool(b.toggle_bookmark("coal")).is_true()
	assert_bool(b.toggle_bookmark("no_such_item")).is_false()
	assert_array(b.bookmarks).is_equal(["iron_ingot", "coal"])
	assert_bool(b.toggle_bookmark("iron_ingot")).is_false()
	assert_bool(b.is_bookmarked("iron_ingot")).is_false()
	b.set_bookmarks(["coal", "gone_item", "coal", "furnace"])
	assert_array(b.bookmarks).is_equal(["coal", "furnace"])
	var b2 := _through_player_save(b)
	assert_array(b2.bookmarks).is_equal(["coal", "furnace"])
	# the old user:// list (before ADR 0115) still loads as the start of a world saved without bookmarks
	var path := "user://test_bookmarks.json"
	var f := FileAccess.open(path, FileAccess.WRITE)
	f.store_string(JSON.stringify({"bookmarks": ["glass", "gone_item"]}))
	f.close()
	b2.load_bookmarks(path)
	assert_array(b2.bookmarks).is_equal(["glass"])
	DirAccess.remove_absolute(ProjectSettings.globalize_path(path))
	b2.load_bookmarks("user://missing_bookmarks.json")   # a missing file keeps what is there
	assert_array(b2.bookmarks).is_equal(["glass"])


## What main._player_dict / _load_player do with the bookmarks (ADR 0115): out as JSON, back through set_bookmarks.
func _through_player_save(from: RecipeBrowser) -> RecipeBrowser:
	var d = JSON.parse_string(JSON.stringify({"version": 1, "bookmarks": from.bookmarks}))
	var to := RecipeBrowser.new(items, recipes)
	to.set_bookmarks(d["bookmarks"])
	return to


# ---------- R / U in the world, station tabs, recipe bookmarks (ADR 0093) ----------

func test_lookup_takes_the_aimed_item_then_the_held_one() -> void:
	assert_str(b.lookup(["glass", "stick"], "recipes")).is_equal("glass")
	assert_str(b.lookup(["", "glass"], "recipes")).is_equal("glass")      # nothing aimed at: the held item
	assert_str(b.lookup(["no_such_item", "stick"], "uses")).is_equal("stick")
	assert_str(b.lookup(["", ""], "recipes")).is_empty()


func test_show_recipe_opens_at_that_recipe_and_tabs_select() -> void:
	assert_bool(b.show_recipe("cook_meat_firepit")).is_true()
	assert_str(b.current_recipe()["id"]).is_equal("cook_meat_firepit")
	var cats: Array = b.view["categories"]
	assert_int(cats.size()).is_greater(1)   # furnace and firepit: a tab each
	var other := 0 if b.view["cat"] != 0 else 1
	b.select_category(other)
	assert_int(b.view["cat"]).is_equal(other)
	assert_int(b.view["index"]).is_equal(0)
	assert_bool(b.show_recipe("no_such_recipe")).is_false()


func test_recipe_bookmarks_persist() -> void:
	var mark := RecipeBrowser.RECIPE_MARK + "smelt_glass"
	assert_bool(b.toggle_bookmark(mark)).is_true()
	assert_bool(b.toggle_bookmark(RecipeBrowser.RECIPE_MARK + "gone")).is_false()
	b.toggle_bookmark("coal")
	assert_array(_through_player_save(b).bookmarks).is_equal([mark, "coal"])


func test_main_saves_bookmarks_with_the_player() -> void:
	var src := FileAccess.get_file_as_string("res://scripts/main.gd")
	assert_str(src).contains('"bookmarks": inv_screen.browser.bookmarks')
	assert_str(src).contains('inv_screen.browser.set_bookmarks(d["bookmarks"])')


# ---------- machine recipes as data (ADR 0115) ----------

func test_process_recipes_tab_under_their_station() -> void:
	assert_bool(b.show("crushed_iron_ore", "recipes")).is_true()
	assert_str(b.current_category()["id"]).is_equal("station:ore_crusher")
	assert_str(b.category_title("station:ore_crusher")).is_equal("Ore Crusher")
	assert_array(b.catalysts("station:ore_crusher")).is_equal(["ore_crusher"])
	var l := b.layout(b.current_recipe())
	assert_str(l["needs"]).is_equal("Ore Crusher")
	assert_str(l["info"]).is_equal("1.0 s, steam from a boiler")
	assert_int(l["inputs"].size()).is_equal(1)
	assert_int(int(l["output"]["count"])).is_equal(2)
	# the machine's uses list every recipe it runs; the ore's uses include the crusher
	assert_bool(b.show("ore_crusher", "uses")).is_true()
	assert_int(b.current_category()["recipes"].size()).is_equal(3)
	assert_bool(b.show("tin_ore", "uses")).is_true()
	assert_bool(b.view["categories"].any(func(c: Dictionary) -> bool: return c["id"] == "station:ore_crusher")).is_true()


func test_a_new_machine_plugs_in_by_data_alone() -> void:
	var r := RecipeRegistry.new(items)
	assert_bool(r.load_json(JSON.stringify({"recipes": [
		{"id": "press_plate", "type": "process", "station": "steam_hammer", "ingredients": ["iron_ingot", "iron_ingot"],
			"output": {"id": "iron_plate", "count": 1}, "seconds": 2.5}]}))).is_true()
	var br := RecipeBrowser.new(items, r)
	assert_bool(br.show("iron_plate", "recipes")).is_true()
	assert_str(br.current_category()["id"]).is_equal("station:steam_hammer")
	assert_str(br.layout(br.current_recipe())["info"]).is_equal("2.5 s")
	assert_array(r.uses_of("iron_ingot")).contains(["press_plate"])
	var bad := RecipeRegistry.new(items)
	assert_bool(bad.load_json(JSON.stringify({"recipes": [
		{"id": "x", "type": "process", "station": "no_such_machine", "ingredients": ["iron_ingot"], "output": {"id": "iron_plate"}}]}))).is_false()


func test_crusher_recipes_match_the_crusher() -> void:
	# the crusher runs on each ore's `crushed` field (ADR 0099); its recipe data must say the same
	for id in items.all_ids():
		var crushed := String(items.get_def(id).get("crushed", ""))
		if crushed == "":
			continue
		var hit := recipes.of_type("process").filter(func(x: Dictionary) -> bool:
			return x["station"] == "ore_crusher" and x["ingredients"] == [id])
		assert_int(hit.size()).override_failure_message("no crusher recipe for " + id).is_equal(1)
		assert_str(hit[0]["output"]["id"]).is_equal(crushed)
		assert_int(int(hit[0]["output"]["count"])).is_equal(2)
		assert_float(float(hit[0]["seconds"])).is_equal(Settings.ore_crusher_crush_s)
