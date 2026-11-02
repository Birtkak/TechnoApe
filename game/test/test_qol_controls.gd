extends GdUnitTestSuite

## ADR 0130: toggle sprint and sneak, Alt + wheel row cycling, double click gathering and shift +
## double click moving, the coordinates line and the hurt / cold edge.

var items: ItemRegistry
var recipes: RecipeRegistry
var p: PlayerInventory
var _saved := {}


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	p = PlayerInventory.new(items, recipes)
	for k in ["toggle_sprint", "toggle_sneak", "screen_effects"]:
		_saved[k] = Settings.get(k)


func after_test() -> void:
	for k in _saved:
		Settings.set(k, _saved[k])


func test_cycle_rows_turns_columns_and_keeps_favourites() -> void:
	p.inv.set_slot(0, {"id": "torch", "count": 1})     # hotbar
	p.inv.set_slot(9, {"id": "flint", "count": 2})     # first main row
	p.inv.set_slot(27, {"id": "stick", "count": 3})    # bottom row
	p.inv.set_slot(1, {"id": "torch", "count": 4})
	p.toggle_favourite(10)                             # column 1 stays
	p.cycle_rows(1)
	assert_str(String(p.inv.get_slot(0).get("id", ""))).is_equal("flint")
	assert_str(String(p.inv.get_slot(27).get("id", ""))).is_equal("torch")
	assert_str(String(p.inv.get_slot(18).get("id", ""))).is_equal("stick")
	assert_int(int(p.inv.get_slot(1).get("count", 0))).is_equal(4)
	p.cycle_rows(-1)
	assert_str(String(p.inv.get_slot(0).get("id", ""))).is_equal("torch")
	assert_str(String(p.inv.get_slot(27).get("id", ""))).is_equal("stick")


func test_double_click_gathers_onto_the_cursor() -> void:
	p.cursor = {"id": "flint", "count": 5}
	p.inv.set_slot(3, {"id": "flint", "count": 10})
	p.inv.set_slot(20, {"id": "flint", "count": 10})
	p.inv.set_slot(21, {"id": "flint", "count": 7})
	p.inv.set_slot(22, {"id": "stick", "count": 7})
	p.toggle_favourite(21)
	p.collect_to_cursor()
	assert_int(int(p.cursor["count"])).is_equal(25)
	assert_bool(p.inv.is_empty_slot(3)).is_true()
	assert_int(int(p.inv.get_slot(21)["count"])).is_equal(7)   # the favourite kept its own
	assert_int(int(p.inv.get_slot(22)["count"])).is_equal(7)


func test_shift_double_click_moves_every_stack_of_the_item() -> void:
	p.inv.set_slot(12, {"id": "flint", "count": 3})
	p.inv.set_slot(30, {"id": "flint", "count": 4})
	p.inv.set_slot(31, {"id": "stick", "count": 4})
	p.shift_all("inv", "flint")
	var hot := 0
	for i in PlayerInventory.HOTBAR:
		if String(p.inv.get_slot(i).get("id", "")) == "flint":
			hot += int(p.inv.get_slot(i)["count"])
	assert_int(hot).is_equal(7)
	assert_str(String(p.inv.get_slot(31).get("id", ""))).is_equal("stick")


func test_toggle_sneak_and_sprint_latch() -> void:
	var pl: Player = auto_free(Player.new())
	pl.flying = false
	Settings.toggle_sneak = true
	Settings.toggle_sprint = true
	var ev := InputEventAction.new()
	ev.action = "sneak"
	ev.pressed = true
	pl._unhandled_input(ev)
	assert_bool(pl.sneaking()).is_true()
	pl._unhandled_input(ev)
	assert_bool(pl.sneaking()).is_false()
	ev.action = "sprint"
	pl._unhandled_input(ev)
	assert_bool(pl.sprint_held()).is_true()
	Settings.toggle_sprint = false
	assert_bool(pl.sprint_held()).is_false()   # held mode reads the key, which is up


func test_coords_text_and_compass() -> void:
	var t := GameHud.coords_text(Vector3(80, 16, -8), 0.0)
	assert_str(t).contains("X 10.0").contains("Y 2.0").contains("Z -1.0").contains("  N 0")
	assert_str(GameHud.coords_text(Vector3.ZERO, -PI / 2.0)).contains(" E 90")   # facing +X
	assert_str(GameHud.coords_text(Vector3.ZERO, PI)).contains(" S 180")


func test_hurt_edge_flashes_and_obeys_the_setting() -> void:
	var hud: GameHud = auto_free(GameHud.new())
	add_child(hud)
	hud._on_health(20)
	assert_float(hud.edge_color().a).is_equal(0.0)
	Settings.screen_effects = true
	hud._on_health(16)
	assert_float(hud.edge_color().a).is_greater(0.3)
	Settings.screen_effects = false
	assert_float(hud.edge_color().a).is_equal(0.0)


func test_craft_button_makes_it_into_the_inventory() -> void:
	p.set_crafting_grid(3, "crafting_table")
	p.pickup("oak_planks", 18 * items.units("oak_planks"))
	var res := p.craft_recipe(recipes.recipes["chest"], false)
	assert_str(res["error"]).is_empty()
	assert_int(p.inv.count_of("chest")).is_equal(1)
	assert_int(p.inv.count_of("oak_planks")).is_equal(10 * items.units("oak_planks"))   # 8 blocks went in
	for i in p.craft.grid.size():
		assert_bool(p.craft.grid.is_empty_slot(i)).is_true()
	p.craft_recipe(recipes.recipes["chest"], true)   # Shift: as many as the planks make
	assert_int(p.inv.count_of("chest")).is_equal(2)
	assert_str(p.craft_recipe(recipes.recipes["chest"], false)["error"]).is_not_empty()


func test_tooltip_facts() -> void:
	assert_str(InventoryScreen.item_facts(items, recipes, "oak_log", false)).contains("Fuel: burns 15 s, smelts 1.5")
	assert_str(InventoryScreen.item_facts(items, recipes, "cooked_meat", false)).contains("Food +6")
	assert_str(InventoryScreen.item_facts(items, recipes, "chest", false)).contains("Made by: Crafting Table")
	assert_str(InventoryScreen.item_facts(items, recipes, "oak_planks", false)).contains("Used in")
	assert_str(InventoryScreen.item_facts(items, recipes, "oak_log", true)).contains("oak_log").contains("#wood")
