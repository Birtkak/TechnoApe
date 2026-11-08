extends GdUnitTestSuite

## Headless tests for the player model (ADR 0005): hotbar selection, pickup
## order, shift-click between hotbar and main, crafting output, buckets,
## health; and the mining table against the material list and items.

var items: ItemRegistry
var recipes: RecipeRegistry
var mining: MiningTable
var p: PlayerInventory


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	mining = MiningTable.load_default()
	p = PlayerInventory.new(items, recipes)


# ---------- mining table data ----------

func test_mining_table_loads() -> void:
	assert_array(mining.errors).is_empty()
	assert_int(mining.materials.size()).is_greater(10)


func test_mining_table_names_exist() -> void:
	var names := []
	for d in GameMaterials.defs():
		names.append(d["name"])
	for m in mining.materials:
		assert_bool(m in names).override_failure_message("unknown material in mining.json: %s" % m).is_true()
		var e: Dictionary = mining.materials[m]
		if e.has("drops"):
			var ids: Array = e["drops"].map(func(d: Dictionary) -> String: return d["id"]) if e["drops"] is Array else [e["drops"]]
			for id in ids:
				assert_bool(items.has(id)).override_failure_message("unknown drop item %s for %s" % [id, m]).is_true()


func test_block_items_reference_materials() -> void:
	var names := []
	for d in GameMaterials.defs():
		names.append(d["name"])
	for id in items.all_ids():
		var d := items.get_def(id)
		if d.has("material"):
			assert_bool(d["material"] in names).override_failure_message("item %s places unknown material %s" % [id, d["material"]]).is_true()


func test_break_times_need_the_right_tool() -> void:
	# stone needs a pickaxe: by hand it takes 2 minutes and drops nothing; the flint pickaxe
	# breaks it in hardness * 3 / 2 but rock drops nothing before copper (drop_tier, ADR 0023)
	assert_float(mining.break_time("stone")).is_equal_approx(120.0, 0.01)
	assert_dict(mining.drops("stone")).is_empty()
	var flint_pick := items.get_def("flint_pickaxe")
	var copper_pick := items.get_def("copper_pickaxe")
	assert_float(mining.break_time("stone", flint_pick)).is_equal_approx(2.25, 0.01)
	assert_float(mining.break_time("stone", copper_pick)).is_equal_approx(1.5, 0.01)   # tier speed 3 (ADR 0139)
	assert_dict(mining.drops("stone", flint_pick)).is_empty()
	assert_dict(mining.drops("stone", copper_pick)).is_equal({"id": "cobblestone", "count": 1})
	# dirt wants a shovel: by hand or with a pickaxe it is slow (still drops), a shovel is quick
	assert_float(mining.break_time("dirt")).is_equal_approx(40.0, 0.01)
	assert_float(mining.break_time("dirt", flint_pick)).is_equal_approx(40.0, 0.01)
	assert_float(mining.break_time("dirt", items.get_def("flint_shovel"))).is_equal_approx(0.75, 0.01)
	# logs and planks want an axe; the right tool never takes under 0.5 s or over 10 s
	assert_float(mining.break_time("wood")).is_equal_approx(120.0, 0.01)
	assert_float(mining.break_time("planks")).is_equal_approx(120.0, 0.01)
	assert_float(mining.break_time("wood", items.get_def("flint_axe"))).is_equal_approx(3.0, 0.01)
	assert_float(mining.break_time("snow", items.get_def("copper_shovel"))).is_equal_approx(0.5, 0.01)
	assert_float(mining.break_time("obsidian", items.get_def("iron_pickaxe"))).is_between(0.5, 10.0)
	# copper ore below its min_tier counts as the wrong tool
	assert_float(mining.break_time("copper_ore", flint_pick)).is_equal_approx(120.0, 0.01)
	# leaves want no tool: quick by hand
	assert_float(mining.break_time("leaves")).is_equal_approx(0.3, 0.01)
	assert_dict(mining.drops("dirt")).is_equal({"id": "dirt", "count": 1})
	# copper ore needs tier 1, iron ore tier 2 (bronze): copper is slow and yields nothing there
	assert_dict(mining.drops("copper_ore", flint_pick)).is_empty()
	assert_dict(mining.drops("copper_ore", copper_pick)).is_equal({"id": "copper_ore", "count": 1})
	assert_dict(mining.drops("iron_ore", copper_pick)).is_empty()
	assert_dict(mining.drops("iron_ore", items.get_def("iron_pickaxe"))).is_equal({"id": "iron_ore", "count": 1})
	# liquids and unknown materials are unbreakable
	assert_float(mining.break_time("water")).is_equal(-1.0)
	assert_float(mining.break_time("bedrock")).is_equal(-1.0)
	assert_float(mining.break_time("tall_grass")).is_equal(0.0)


# ---------- hotbar ----------

func test_bodies_walk_through_plants() -> void:
	# the unstick check and the library agree: plants, pickups and work spots are walked through
	for n in ["tall_grass", "flower", "fern", "reed", "pebbles", "work_spot", "water", "torch"]:
		assert_bool(GameMaterials.blocks_bodies(GameMaterials.defs().map(func(d: Dictionary) -> String: return d["name"]).find(n) + 1)).override_failure_message(n).is_false()
	for n in ["stone", "grass", "sand", "wattle", "storage_pot", "furniture", "stalagmite"]:
		assert_bool(GameMaterials.blocks_bodies(GameMaterials.defs().map(func(d: Dictionary) -> String: return d["name"]).find(n) + 1)).override_failure_message(n).is_true()


func test_flowers_drop_themselves_and_place() -> void:
	for pair in [["flower", "dandelion"], ["flower_red", "poppy"], ["cornflower", "cornflower"], ["oxeye_daisy", "oxeye_daisy"], ["allium", "allium"]]:
		assert_dict(mining.drops(pair[0])).is_equal({"id": pair[1], "count": 1})
		assert_str(String(items.get_def(pair[1]).get("material", ""))).is_equal(pair[0])


func test_select_and_scroll_wrap() -> void:
	p.select(8)
	p.scroll(1)
	assert_int(p.selected).is_equal(0)
	p.scroll(-1)
	assert_int(p.selected).is_equal(8)
	p.select(3)
	assert_int(p.selected).is_equal(3)


func test_pickup_fills_hotbar_first_and_merges() -> void:
	var full := p.items.max_stack("dirt")
	assert_int(p.pickup("dirt", full + 6)).is_equal(0)
	assert_dict(p.inv.get_slot(0)).is_equal({"id": "dirt", "count": full})
	assert_dict(p.inv.get_slot(1)).is_equal({"id": "dirt", "count": 6})
	# a stack in main merges before a new hotbar slot is used
	p.inv.set_slot(20, {"id": "sand", "count": 10})
	p.pickup("sand", 5)
	assert_dict(p.inv.get_slot(20)).is_equal({"id": "sand", "count": 15})
	assert_bool(p.inv.is_empty_slot(2)).is_true()


func test_pickup_leftover_when_full() -> void:
	for i in PlayerInventory.SIZE:
		p.inv.set_slot(i, {"id": "stone", "count": p.items.max_stack("stone")})
	assert_int(p.pickup("stone", 3)).is_equal(3)
	assert_int(p.pickup("dirt", 1)).is_equal(1)


func test_consume_and_drop_held() -> void:
	p.pickup("sand", 3)
	assert_int(p.consume_held(1)).is_equal(1)
	assert_dict(p.held()).is_equal({"id": "sand", "count": 2})
	assert_dict(p.drop_held(false)).is_equal({"id": "sand", "count": 1})
	assert_dict(p.drop_held(true)).is_equal({"id": "sand", "count": 1})
	assert_dict(p.held()).is_empty()
	assert_dict(p.drop_held(true)).is_empty()


func test_bucket_fills_and_empties() -> void:
	p.pickup("bucket", 2)
	var def := p.held_def()
	assert_str(def["fills"]["water"]).is_equal("water_bucket")
	assert_int(p.replace_held_one("water_bucket")).is_equal(0)
	assert_dict(p.held()).is_equal({"id": "bucket", "count": 1})
	assert_int(p.inv.count_of("water_bucket")).is_equal(1)
	p.select(1)
	assert_dict(p.held()).is_equal({"id": "water_bucket", "count": 1})
	p.replace_held_one("bucket")
	assert_dict(p.held()).is_equal({"id": "bucket", "count": 1})


# ---------- inventory screen ----------

func test_shift_click_moves_between_hotbar_and_main() -> void:
	p.inv.set_slot(2, {"id": "oak_log", "count": 10})
	p.click_slot(2, MOUSE_BUTTON_LEFT, true)
	assert_bool(p.inv.is_empty_slot(2)).is_true()
	assert_dict(p.inv.get_slot(9)).is_equal({"id": "oak_log", "count": 10})
	p.click_slot(9, MOUSE_BUTTON_LEFT, true)
	assert_dict(p.inv.get_slot(0)).is_equal({"id": "oak_log", "count": 10})


func test_mouse_tweaks_drags() -> void:
	# right drag: one item into each slot passed, skipping slots of another item
	p.inv.set_slot(12, {"id": "stick", "count": 1})
	p.cursor = {"id": "flint", "count": 5}
	for i in [10, 11, 12]:
		p.drag_place_one("inv", i)
	assert_dict(p.inv.get_slot(10)).is_equal({"id": "flint", "count": 1})
	assert_dict(p.inv.get_slot(11)).is_equal({"id": "flint", "count": 1})
	assert_dict(p.inv.get_slot(12)).is_equal({"id": "stick", "count": 1})
	assert_int(int(p.cursor["count"])).is_equal(3)
	# left drag: the same item comes back into the cursor, others stay
	for i in [10, 11, 12]:
		p.drag_collect("inv", i)
	assert_int(int(p.cursor["count"])).is_equal(5)
	assert_bool(p.inv.is_empty_slot(10)).is_true()
	assert_dict(p.inv.get_slot(12)).is_equal({"id": "stick", "count": 1})
	# shift drag: each slot is shift-clicked (main -> hotbar)
	p.cursor = {}
	p.drag_shift("inv", 12)
	assert_dict(p.inv.get_slot(0)).is_equal({"id": "stick", "count": 1})


func test_mouse_tweaks_wheel() -> void:
	p.inv.set_slot(0, {"id": "dirt", "count": 3})
	p.inv.set_slot(20, {"id": "dirt", "count": 2})
	# wheel down on a hotbar slot: one item to the main grid, merging first
	p.wheel_move("inv", 0, true)
	assert_dict(p.inv.get_slot(0)).is_equal({"id": "dirt", "count": 2})
	assert_dict(p.inv.get_slot(20)).is_equal({"id": "dirt", "count": 3})
	# wheel up: one item pulled back from the main grid
	p.wheel_move("inv", 0, false)
	p.wheel_move("inv", 0, false)
	assert_dict(p.inv.get_slot(0)).is_equal({"id": "dirt", "count": 4})
	assert_dict(p.inv.get_slot(20)).is_equal({"id": "dirt", "count": 1})


func test_cursor_left_right_click() -> void:
	p.inv.set_slot(0, {"id": "flint", "count": 9})
	p.click_slot(0, MOUSE_BUTTON_RIGHT, false)   # pick up half (5)
	assert_dict(p.cursor).is_equal({"id": "flint", "count": 5})
	p.click_slot(1, MOUSE_BUTTON_RIGHT, false)   # place one
	assert_dict(p.inv.get_slot(1)).is_equal({"id": "flint", "count": 1})
	p.click_slot(2, MOUSE_BUTTON_LEFT, false)    # place the rest
	assert_dict(p.cursor).is_empty()
	assert_dict(p.inv.get_slot(2)).is_equal({"id": "flint", "count": 4})


## Logs and planks count in voxels; a right click moves one block's worth (ADR 0034).
func test_personal_grid_crafts_planks_and_sticks() -> void:
	var u := p.items.units("oak_log")
	p.inv.set_slot(0, {"id": "oak_log", "count": 2 * u})
	p.click_slot(0, MOUSE_BUTTON_LEFT, false)
	p.click_craft_slot(0, MOUSE_BUTTON_RIGHT, false)   # one log into the grid
	assert_dict(p.craft.preview_output()).is_equal({"id": "oak_planks", "count": 2 * u})   # 2 per log by hand (ADR 0019)
	p.click_slot(0, MOUSE_BUTTON_LEFT, false)           # cursor (1 log) back to slot 0
	p.click_craft_output(false)
	assert_dict(p.cursor).is_equal({"id": "oak_planks", "count": 2 * u})
	assert_bool(p.craft.grid.is_empty_slot(0)).is_true()
	# planks vertical in the 2x2 -> sticks via shift-click straight into the inventory
	p.click_craft_slot(0, MOUSE_BUTTON_RIGHT, false)
	p.click_craft_slot(2, MOUSE_BUTTON_RIGHT, false)
	assert_dict(p.cursor).is_empty()                    # both planks went into the grid
	assert_dict(p.craft.preview_output()).is_equal({"id": "stick", "count": 4})
	p.click_craft_output(true)
	assert_int(p.inv.count_of("stick")).is_equal(4)
	assert_dict(p.craft.preview_output()).is_empty()


func test_close_screen_returns_grid_and_cursor() -> void:
	p.inv.set_slot(0, {"id": "oak_log", "count": 3})
	p.click_slot(0, MOUSE_BUTTON_LEFT, false)
	p.click_craft_slot(1, MOUSE_BUTTON_RIGHT, false)
	var dropped := p.close_screen()
	assert_array(dropped).is_empty()
	assert_dict(p.cursor).is_empty()
	assert_int(p.inv.count_of("oak_log")).is_equal(3)


func test_crafting_table_grid_swap() -> void:
	p.inv.set_slot(0, {"id": "flint", "count": 8})
	p.click_slot(0, MOUSE_BUTTON_LEFT, false)
	p.click_craft_slot(0, MOUSE_BUTTON_RIGHT, false)
	assert_array(p.set_crafting_grid(3, "crafting_table")).is_empty()
	assert_int(p.craft.width).is_equal(3)
	assert_int(p.inv.count_of("flint")).is_equal(1)   # the grid's flint came back


# ---------- health ----------

func test_damage_and_respawn() -> void:
	p.damage(7)
	assert_int(p.health).is_equal(13)
	p.damage(100)
	assert_bool(p.is_dead()).is_true()
	p.respawn()
	assert_int(p.health).is_equal(20)
	p.heal(5)
	assert_int(p.health).is_equal(20)


func test_summary_is_serialisable() -> void:
	p.pickup("dirt", 3)
	var s := p.summary()
	assert_str(JSON.stringify(s)).contains("dirt")
	assert_int(s["health"]).is_equal(20)


func test_only_physical_damage_bleeds() -> void:
	for c in ["starvation", "hypothermia", "heatstroke", "poison", "smoke", "drowning", "lightning", "hot metal in bare hands"]:
		assert_bool(PlayerInventory.bleeds(c)).override_failure_message(c).is_false()
	for c in ["wolf attack", "explosion", "impact (fall 6 m)", "cave_in", "a bursting boiler"]:
		assert_bool(PlayerInventory.bleeds(c)).override_failure_message(c).is_true()
