extends GdUnitTestSuite

## Crafting in the world (ADR 0042): per-stack data (wear, heat, work pieces) through inventory moves,
## tool wear, tools assembled on the ground, the firepit worked by hand, and smithing on the anvil.

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	store = BlockEntities.new(items, recipes)
	Inventory.clock_s = 1000.0


func after_test() -> void:
	Inventory.clock_s = 0.0


func _ticks(n: int) -> void:
	for i in n:
		Inventory.clock_s += BlockEntities.TICK_S
		store.tick()


# ---------- data ----------

func test_data_loads_clean() -> void:
	assert_array(items.errors).is_empty()
	assert_array(recipes.errors).is_empty()
	for t in ["assemble", "smith"]:
		assert_array(recipes.of_type(t)).override_failure_message("no %s recipes" % t).is_not_empty()


func test_items_with_identity_data_stack_to_one() -> void:
	# wear and work pieces never merge, so whatever carries them must not stack (Inventory.merge_into)
	for d in items.items.values():
		if d.has("durability") or "work_piece" in d.get("tags", []):
			assert_int(items.max_stack(d["id"])).override_failure_message("%s stacks" % d["id"]).is_equal(1)
		if d.has("work_piece"):
			assert_bool(items.has(d["work_piece"]) and items.has_tag(d["id"], "heatable")).override_failure_message(d["id"]).is_true()


## Flint, copper, bronze and iron tools are assembled on the ground (ADR 0042, 0070); the firestarter
## is a simple grid craft.
func test_no_age_tool_is_made_in_the_grid() -> void:
	for r in recipes.recipes.values():
		if r["type"] in ["shaped", "shapeless"]:
			var d := items.get_def(r["output"]["id"])
			var age_tool: bool = "tool" in d.get("tags", []) and d.has("tier")
			assert_bool(age_tool).override_failure_message(r["id"]).is_false()


func test_every_smith_shape_fits_its_ingots() -> void:
	for r in recipes.of_type("smith"):
		var cells := AnvilEntity.cells_of(r)
		assert_int(cells).override_failure_message(r["id"]).is_greater(0)
		assert_int(cells).override_failure_message(r["id"]).is_less_equal(AnvilEntity.ingots_for(r) * AnvilEntity.CELLS_PER_INGOT)
		for z in 8:
			for x in 8:
				assert_int(AnvilEntity.target_of(r, x, z)).is_less_equal(AnvilEntity.MAX_H)


# ---------- per-stack data ----------

func test_wear_and_work_survive_every_move() -> void:
	var a := Inventory.new(items, 4)
	var b := Inventory.new(items, 4)
	a.set_slot(0, {"id": "flint_axe", "count": 1, "wear": 7})
	a.set_slot(1, {"id": "copper_work_piece", "count": 1, "work": {"metal": "copper_ingot", "heights": [1, 2]}})
	assert_int(int(a.get_slot(0)["wear"])).is_equal(7)
	var cursor := a.left_click(0, {})
	assert_int(int(cursor["wear"])).is_equal(7)
	cursor = b.left_click(2, cursor)
	assert_dict(cursor).is_empty()
	assert_int(int(b.get_slot(2)["wear"])).is_equal(7)
	assert_bool(a.shift_click(1, b)).is_true()
	var w: Dictionary = b.get_slot(0)
	assert_str(w["id"]).is_equal("copper_work_piece")
	assert_array(w["work"]["heights"]).is_equal([1, 2])
	# keys that do not apply are dropped: no spoil on a tool, no wear at 0
	b.set_slot(3, {"id": "flint_knife", "count": 1, "spoil": 0.5, "wear": 0})
	assert_dict(b.get_slot(3)).is_equal({"id": "flint_knife", "count": 1})


func test_player_pickup_keeps_the_data() -> void:
	var p := PlayerInventory.new(items, recipes)
	p.pickup("copper_hammer", 1, {"wear": 12})
	assert_int(int(p.inv.get_slot(0)["wear"])).is_equal(12)
	p.pickup("copper_ingot", 2, {"hot_until": Inventory.clock_s + 20.0})
	assert_float(Inventory.heat_of(p.inv.get_slot(1))).is_equal_approx(0.5, 1e-4)


func test_heat_cools_and_averages_on_merge() -> void:
	var inv := Inventory.new(items, 2)
	inv.add("copper_ingot", 1, {"hot_until": Inventory.clock_s + Settings.metal_cool_s})
	assert_float(Inventory.heat_of(inv.get_slot(0))).is_equal_approx(1.0, 1e-4)
	inv.add("copper_ingot", 1)   # a cold one joins: half the heat left on average
	assert_int(int(inv.get_slot(0)["count"])).is_equal(2)
	assert_float(Inventory.heat_of(inv.get_slot(0))).is_equal_approx(0.5, 1e-4)
	Inventory.clock_s += Settings.metal_cool_s
	assert_float(Inventory.heat_of(inv.get_slot(0))).is_equal(0.0)
	var t := inv.take(0, 1)
	assert_bool(t.has("hot_until")).is_false()   # cold: the key is gone, so it stacks with any cold ingot


# ---------- tool wear ----------

func test_a_tool_wears_out_and_breaks() -> void:
	var p := PlayerInventory.new(items, recipes)
	var broke := []
	p.tool_broke.connect(func(id: String) -> void: broke.append(id))
	p.inv.set_slot(0, {"id": "flint_knife", "count": 1})
	var dur := int(items.get_def("flint_knife")["durability"])
	assert_bool(p.wear_held(dur - 1)).is_false()
	assert_int(Inventory.uses_left(items, p.held())).is_equal(1)
	assert_bool(p.wear_held(1)).is_true()
	assert_dict(p.held()).is_empty()
	assert_array(broke).is_equal(["flint_knife"])
	p.inv.set_slot(0, {"id": "stick", "count": 3})
	assert_bool(p.wear_held(5)).is_false()   # no durability: nothing happens
	assert_int(int(p.held()["count"])).is_equal(3)


func test_a_tool_kept_in_the_grid_wears() -> void:
	var g := CraftingGrid.new(items, recipes, 3, "")
	g.grid.set_slot(0, {"id": "oak_log", "count": items.units("oak_log")})
	g.grid.set_slot(1, {"id": "flint_axe", "count": 1})
	assert_str(g.craft_once()["id"]).is_equal("firewood")
	assert_int(int(g.grid.get_slot(1)["wear"])).is_equal(1)
	assert_array(g.tools_broken).is_empty()


func test_a_thrown_spear_can_break() -> void:
	var dur := int(items.get_def("flint_spear")["durability"])
	var s := Inventory.wear_stack(items, {"id": "flint_spear", "count": 1, "wear": dur - 1})
	assert_bool(s.get("broken", false)).is_true()


# ---------- assembly ----------

func test_assembly_goes_part_by_part_and_comes_apart() -> void:
	assert_bool(AssemblyEntity.starts(recipes, "flint_axe_head")).is_true()
	assert_bool(AssemblyEntity.starts(recipes, "stick")).is_false()
	var a := store.create(Vector3i(0, 10, 0), "assembly") as AssemblyEntity
	a.parts.append("flint_axe_head")
	assert_array(a.next_specs()).is_equal(["#stick"])
	var r := a.use(items.get_def("cordage"))
	assert_int(r["consume"]).is_equal(0)   # the binding comes after the handle
	assert_str(r["msg"]).contains("stick")
	r = a.use(items.get_def("stick"))
	assert_int(r["consume"]).is_equal(1)
	assert_dict(r["done"]).is_empty()
	r = a.use({})   # an empty hand takes the last part back
	assert_array(r["give"]).is_equal([{"id": "stick", "count": 1}])
	assert_bool(r["remove"]).is_false()
	a.use(items.get_def("stick"))
	assert_array(a.contents()).is_equal([{"id": "flint_axe_head", "count": 1}, {"id": "stick", "count": 1}])
	r = a.use(items.get_def("leather_strip"))
	assert_dict(r["done"]).is_equal({"id": "flint_axe", "count": 1})
	assert_bool(r["remove"]).is_true()


func test_assembly_saves_and_loads() -> void:
	var a := store.create(Vector3i(0, 10, 0), "assembly") as AssemblyEntity
	a.parts.append_array(["flint_spear_head", "stick"])
	var b := BlockEntities.new(items, recipes)
	assert_bool(b.load_dict(store.to_dict())).is_true()
	assert_array((b.at(Vector3i(0, 10, 0)) as AssemblyEntity).parts).is_equal(["flint_spear_head", "stick"])
	assert_array((b.at(Vector3i(0, 10, 0)) as AssemblyEntity).view_parts()["items"]).has_size(2)


## The firepit is laid on the ground like a tool (ADR 0083): four stones in a ring, then two sticks; it is
## no grid recipe any more.
func test_firepit_is_laid_stone_by_stone() -> void:
	assert_bool(AssemblyEntity.starts(recipes, "loose_stone")).is_true()
	assert_bool(AssemblyEntity.starts(recipes, "granite_stone")).is_true()
	assert_str(recipes.recipes["firepit"]["type"]).is_equal("assemble")
	var a := store.create(Vector3i(0, 10, 0), "assembly") as AssemblyEntity
	a.parts.append("loose_stone")
	assert_str(a.use(items.get_def("stick"))["msg"]).contains("rock")   # the ring comes first
	for i in 3:
		assert_int(a.use(items.get_def("slate_stone"))["consume"]).is_equal(1)
	assert_dict(a.use(items.get_def("stick"))["done"]).is_empty()
	var r := a.use(items.get_def("stick"))
	assert_dict(r["done"]).is_equal({"id": "firepit", "count": 1})
	assert_str(String(items.get_def("firepit").get("work", ""))).is_equal("firepit")   # WorkSystem builds it in place
	var ring: Array = a.view_parts()["items"]
	assert_float((ring[0]["at"] as Vector3).distance_to(ring[2]["at"])).is_greater(0.5)   # stones lie apart, round the sticks


# ---------- the firepit by hand ----------

func test_firepit_by_hand_cooks_and_hands_back() -> void:
	var f := store.create(Vector3i(0, 10, 0), "firepit") as FirepitEntity
	var r := f.use({}, {})
	assert_str(r["msg"]).contains("fuel")
	r = f.use({"id": "firestarter", "count": 1}, items.get_def("firestarter"))
	assert_bool(r["ignited"]).is_false()
	for i in 3:
		assert_int(f.use({"id": "oak_log", "count": 64}, items.get_def("oak_log"))["consume"]).is_equal(items.units("oak_log"))
	assert_int(f.use({"id": "stick", "count": 1}, items.get_def("stick"))["consume"]).is_equal(0)   # one kind of fuel at a time
	assert_int(f.use({"id": "raw_meat", "count": 4, "spoil": 0.2}, items.get_def("raw_meat"))["consume"]).is_equal(1)
	assert_float(float(f.inv.get_slot(FirepitEntity.INPUT)["spoil"])).is_equal_approx(0.2, 1e-6)
	assert_str(f.use({"id": "dirt", "count": 64}, items.get_def("dirt"))["msg"]).contains("does not go")
	r = f.use({"id": "firestarter", "count": 1}, items.get_def("firestarter"))
	assert_bool(r["ignited"]).is_true()
	_ticks(301)
	r = f.use({}, {})
	assert_str(r["give"][0]["id"]).is_equal("cooked_meat")
	assert_bool(f.use({}, {})["give"].is_empty()).is_true()   # lit: the fuel stays on
	var laid: Array = f.view_parts()["items"]
	assert_array(laid).is_empty()


func test_firepit_by_hand_melts_copper_in_the_crucible() -> void:
	var f := store.create(Vector3i(0, 10, 0), "firepit") as FirepitEntity
	assert_str(f.use({"id": "loose_copper_ore", "count": 4}, items.get_def("loose_copper_ore"))["msg"]).contains("crucible")
	assert_int(f.use({"id": "crucible", "count": 1}, items.get_def("crucible"))["consume"]).is_equal(1)
	for i in 4:
		assert_int(f.use({"id": "loose_copper_ore", "count": 4}, items.get_def("loose_copper_ore"))["consume"]).is_equal(1)
	for i in 4:
		f.use({"id": "oak_log", "count": 64}, items.get_def("oak_log"))
	f.use({"id": "firestarter", "count": 1}, items.get_def("firestarter"))
	_ticks(401)
	assert_str(f.use({}, {})["give"][0]["id"]).is_equal("crucible_copper")


func test_firepit_heats_an_ingot() -> void:
	var f := store.create(Vector3i(0, 10, 0), "firepit") as FirepitEntity
	f.use({"id": "oak_log", "count": 64}, items.get_def("oak_log"))
	assert_int(f.use({"id": "copper_ingot", "count": 3}, items.get_def("copper_ingot"))["consume"]).is_equal(1)
	f.ignite()
	_ticks(int(Settings.metal_heat_s * 20.0) + 2)
	var hot: Dictionary = f.use({}, {})["give"][0]
	assert_str(hot["id"]).is_equal("copper_ingot")
	assert_float(Inventory.heat_of(hot)).is_equal_approx(1.0, 0.01)


# ---------- smithing ----------

func _hot_ingot() -> Dictionary:
	return {"id": "copper_ingot", "count": 1, "hot_until": Inventory.clock_s + Settings.metal_cool_s}


func test_smithing_needs_hot_metal_and_moves_cells() -> void:
	var a := store.create(Vector3i(0, 10, 0), "anvil") as AnvilEntity
	assert_str(a.add_ingot({"id": "copper_ingot", "count": 1})).contains("Heat")
	assert_str(a.add_ingot(_hot_ingot())).is_equal("")
	assert_int(a.metal_cells()).is_equal(AnvilEntity.CELLS_PER_INGOT)
	assert_array(a.options().map(func(r: Dictionary) -> String: return r["id"])).contains(["smith_copper_saw_blade"])
	a.set_recipe(recipes.recipes["smith_copper_hammer_head"])
	assert_int(a.height(2, 2)).is_equal(2)
	assert_str(a.hit(2, 2, Vector2i(0, 1))).is_equal("hit")   # pushed away from a player standing north
	assert_int(a.height(2, 2)).is_equal(1)
	assert_int(a.height(2, 3)).is_equal(3)
	assert_str(a.hit(5, 0, Vector2i(0, -1))).is_equal("")   # no metal there
	a.heights[0] = 1
	assert_str(a.hit(0, 0, Vector2i(-1, 0))).is_equal("stuck")   # the edge of the anvil
	assert_str(a.split(0, 0)).is_equal("hit")
	assert_int(a.metal_cells()).is_equal(AnvilEntity.CELLS_PER_INGOT)
	Inventory.clock_s += Settings.metal_cool_s
	assert_str(a.hit(2, 3, Vector2i(0, 1))).is_equal("cold")


func test_smithing_a_hammer_head_to_the_end() -> void:
	var a := store.create(Vector3i(0, 10, 0), "anvil") as AnvilEntity
	a.add_ingot(_hot_ingot())
	a.set_recipe(recipes.recipes["smith_copper_hammer_head"])
	# the lump is columns 2..5 x rows 2..5, two high; the head is rows 3..4 from column 1 to 6
	var result := ""
	for x in range(2, 6):
		result = a.hit(x, 2, Vector2i(0, 1))   # row 2 into row 3
		result = a.hit(x, 2, Vector2i(0, 1))
		result = a.hit(x, 5, Vector2i(0, -1))  # row 5 into row 4
		result = a.hit(x, 5, Vector2i(0, -1))
	for z in [3, 4]:
		# each row is four high over columns 2..5: drive one cell out to each end, refilling from inside
		a.hit(2, z, Vector2i(-1, 0))
		a.hit(3, z, Vector2i(-1, 0))
		result = a.hit(2, z, Vector2i(-1, 0))
		a.hit(5, z, Vector2i(1, 0))
		a.hit(4, z, Vector2i(1, 0))
		result = a.hit(5, z, Vector2i(1, 0))
	assert_str(result).is_equal("done")
	assert_bool(a.is_done()).is_true()
	assert_dict(a.finish()).is_equal({"id": "copper_hammer_head", "count": 1})
	assert_bool(a.has_piece()).is_false()


func test_a_work_piece_comes_off_and_goes_back() -> void:
	var a := store.create(Vector3i(0, 10, 0), "anvil") as AnvilEntity
	a.add_ingot(_hot_ingot())
	a.set_recipe(recipes.recipes["smith_copper_saw_blade"])
	a.hit(2, 2, Vector2i(-1, 0))
	var before := Array(a.heights)
	assert_array(a.contents().map(func(s: Dictionary) -> String: return s["id"])).is_equal(["stone_anvil", "copper_work_piece"])
	var piece := a.take_piece()
	assert_bool(a.has_piece()).is_false()
	var inv := Inventory.new(items, 1)
	inv.set_slot(0, piece)
	piece = inv.take(0)
	assert_str(a.put_piece(piece)).is_equal("")
	assert_array(Array(a.heights)).is_equal(before)
	assert_str(a.recipe["id"]).is_equal("smith_copper_saw_blade")
	assert_str(a.add_ingot(_hot_ingot())).is_equal("")   # more metal on top
	assert_int(a.metal_cells()).is_equal(2 * AnvilEntity.CELLS_PER_INGOT)


func test_anvil_saves_and_loads() -> void:
	var a := store.create(Vector3i(0, 10, 0), "anvil") as AnvilEntity
	a.add_ingot(_hot_ingot())
	a.set_recipe(recipes.recipes["smith_copper_axe_head"])
	a.hit(3, 3, Vector2i(1, 0))
	var b := BlockEntities.new(items, recipes)
	assert_bool(b.load_dict(store.to_dict())).is_true()
	var c := b.at(Vector3i(0, 10, 0)) as AnvilEntity
	assert_array(Array(c.heights)).is_equal(Array(a.heights))
	assert_str(c.recipe["id"]).is_equal("smith_copper_axe_head")
	assert_float(c.heat()).is_greater(0.9)
	assert_bool(c.view_parts().has("ghost")).is_true()
