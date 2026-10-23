extends GdUnitTestSuite

## Headless tests for the deeper Flint Age (ADR 0030): fiber and cordage
## binding flint heads, crafting that keeps a tool or leaves a remainder,
## hides scraped and dried on the ground, the charcoal pit (piles, sealing,
## charring), the pan's own loot table, the bag, the straw bed, the storage
## pot, the new materials and drops.

var items: ItemRegistry
var recipes: RecipeRegistry
var mining: MiningTable
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	mining = MiningTable.load_default()
	store = BlockEntities.new(items, recipes)


func _grid(ids: Array) -> Array:
	var g := ids.duplicate()
	while g.size() < 9:
		g.append("")
	return g


# ---------- data ----------

func test_data_loads_clean() -> void:
	assert_array(items.errors).is_empty()
	assert_array(recipes.errors).is_empty()
	assert_array(mining.errors).is_empty()
	for t in ["scrape", "dry", "charcoal_pit"]:
		assert_array(recipes.of_type(t)).override_failure_message("no %s recipes" % t).is_not_empty()


func test_flint_heads_need_a_binding_copper_heads_do_not() -> void:
	assert_dict(recipes.match_grid(_grid(["flint_axe_head", "stick"]))).is_empty()
	assert_str(recipes.match_grid(_grid(["flint_axe_head", "stick", "cordage"])).get("id", "")).is_equal("flint_axe")
	assert_str(recipes.match_grid(_grid(["flint_knife_blade", "leather_strip", "stick"])).get("id", "")).is_equal("flint_knife")
	assert_str(recipes.match_grid(_grid(["copper_pickaxe_head", "stick"])).get("id", "")).is_equal("copper_pickaxe")
	assert_str(recipes.match_grid(_grid(["plant_fiber", "plant_fiber", "plant_fiber"])).get("id", "")).is_equal("cordage")
	assert_str(recipes.match_grid(_grid(["flint_spear_head", "stick", "stick", "cordage"])).get("id", "")).is_equal("flint_spear")


func test_fiber_drops_by_hand() -> void:
	var grass: Array = mining.entry("tall_grass")["drops"]
	assert_bool(grass.any(func(d: Dictionary) -> bool: return d["id"] == "plant_fiber" and not d.has("tool"))).is_true()
	assert_str(mining.drops("reed", {}).get("id", "")).is_equal("plant_fiber")


func test_sheep_and_pigs_drop_raw_hide() -> void:
	var mobs = JSON.parse_string(FileAccess.get_file_as_string("res://data/mobs.json"))["mobs"]
	for sp in ["sheep", "pig"]:
		assert_bool(mobs[sp]["drops"].any(func(d: Dictionary) -> bool: return d["id"] == "raw_hide")).override_failure_message(sp).is_true()


# ---------- crafting: keep and remainder ----------

func test_splitting_firewood_keeps_the_axe() -> void:
	var g := CraftingGrid.new(items, recipes, 2, "")
	var u := items.units("oak_log")   # a log is a block's worth of wood voxels (ADR 0034)
	g.grid.set_slot(0, {"id": "oak_log", "count": 2 * u})
	g.grid.set_slot(1, {"id": "flint_axe", "count": 1})
	assert_dict(g.craft_once()).is_equal({"id": "firewood", "count": 4})
	assert_dict(g.grid.get_slot(1)).is_equal({"id": "flint_axe", "count": 1})
	assert_int(int(g.grid.get_slot(0)["count"])).is_equal(u)
	var all := g.craft_all()
	assert_dict(all).is_equal({"id": "firewood", "count": 4})
	assert_bool(g.grid.is_empty_slot(0)).is_true()
	assert_dict(g.grid.get_slot(1)).is_equal({"id": "flint_axe", "count": 1})


func test_serving_stew_gives_the_pot_back() -> void:
	var g := CraftingGrid.new(items, recipes, 2, "")
	g.grid.set_slot(0, {"id": "pot_of_stew", "count": 1})
	for i in [1, 2, 3]:
		g.grid.set_slot(i, {"id": "clay_bowl", "count": 1})
	assert_dict(g.craft_once()).is_equal({"id": "stew", "count": 3})
	assert_dict(g.grid.get_slot(0)).is_equal({"id": "cooking_pot", "count": 1})
	assert_bool(g.grid.is_empty_slot(1)).is_true()
	assert_str(items.get_def("stew").get("empties_to", "")).is_equal("clay_bowl")


func test_cooking_pot_makes_stew_in_the_firepit() -> void:
	var f: FirepitEntity = store.create(Vector3i(0, 0, 0), "firepit")
	assert_bool(f.may_place(FirepitEntity.VESSEL, "cooking_pot")).is_true()
	f.inv.set_slot(FirepitEntity.VESSEL, {"id": "cooking_pot", "count": 1})
	f.inv.set_slot(FirepitEntity.INPUT, {"id": "raw_meat", "count": 2})
	assert_str(f.current_recipe().get("id", "")).is_equal("stew_meat")


# ---------- hides ----------

func _ground(item: String) -> GroundEntity:
	var g := store.create(Vector3i(0, 10, 0), "ground") as GroundEntity
	g.item = item
	return g


func test_hide_scrapes_with_a_knife_then_dries_into_leather() -> void:
	var g := _ground("raw_hide")
	assert_str(g.use(items.get_def("stick"))["msg"]).contains("knife")
	for i in 7:
		assert_bool(g.use(items.get_def("flint_knife")).get("stroke", false)).is_true()
	assert_str(g.item).is_equal("raw_hide")
	g.use(items.get_def("flint_knife"))
	assert_str(g.item).is_equal("scraped_hide")
	for i in 90 * 20 - 1:
		store.tick()
	assert_str(g.item).is_equal("scraped_hide")
	store.tick()
	assert_str(g.item).is_equal("leather")
	var res := g.use({})
	assert_array(res["give"]).is_equal([{"id": "leather", "count": 1}])


func test_bear_hide_gives_two_leather_and_saves_its_progress() -> void:
	var g := _ground("bear_hide")
	for i in 12:
		g.use(items.get_def("copper_knife"))
	assert_str(g.item).is_equal("scraped_hide")
	assert_int(g.count).is_equal(2)
	for i in 600:
		store.tick()
	var d := store.to_dict()
	var b2 := BlockEntities.new(items, recipes)
	assert_bool(b2.load_dict(JSON.parse_string(JSON.stringify(d)))).is_true()
	var g2: GroundEntity = b2.at(Vector3i(0, 10, 0))
	assert_int(g2.dry_t).is_equal(600)
	for i in 90 * 20 - 600:
		b2.tick()
	assert_array(g2.contents()).is_equal([{"id": "leather", "count": 2}])


# ---------- the charcoal pit ----------

class FakeMain:
	extends Node
	var blocks: BlockEntities
	var voxels := {}
	func get_world_voxel(p: Vector3i) -> int:
		return voxels.get(p, GameMaterials.AIR)


func _pit_world() -> Array:
	var m := FakeMain.new()
	m.blocks = store
	var w := WorkSystem.new()
	w.main = m
	return [m, w]


func test_piles_fill_to_sixteen_and_char_only_when_sealed() -> void:
	var e: FirewoodPileEntity = store.create(Vector3i(0, 0, 0), "firewood_pile")
	assert_int(e.add(40)).is_equal(Settings.firewood_per_pile - 1)
	assert_bool(e.is_full()).is_true()
	e.light()
	for i in int(Settings.charcoal_pit_burn_s * 20.0) + 10:
		store.tick()
	assert_str(e.state).is_equal("lit")   # never sealed: it waits to burn away
	e.sealed = true
	store.tick()
	assert_str(e.state).is_equal("charcoal")
	assert_array(e.contents()).is_equal([{"id": "charcoal", "count": Settings.firewood_per_pile / 2}])


func test_a_pit_is_every_touching_pile_and_seals_with_soil() -> void:
	var mw := _pit_world()
	var m: FakeMain = mw[0]
	var w: WorkSystem = mw[1]
	var a := Vector3i(0, 0, 0)
	var b := Vector3i(1, 0, 0)
	store.create(a, "firewood_pile")
	store.create(b, "firewood_pile")
	m.voxels[a] = GameMaterials.WORK_BLOCK
	m.voxels[b] = GameMaterials.WORK_BLOCK
	var pit := w.pit_of(a)
	assert_int(pit.size()).is_equal(2)
	assert_int(w.open_faces(pit)).is_equal(10)   # 2 x 6 faces minus the shared pair
	for p in pit:
		for d in WorkSystem.DIRS6:
			if not m.voxels.has(p + d):
				m.voxels[p + d] = GameMaterials.DIRT
	assert_int(w.open_faces(pit)).is_equal(0)
	m.voxels[Vector3i(0, 1, 0)] = GameMaterials.TALL_GRASS   # a plant on top does not seal it
	assert_int(w.open_faces(pit)).is_equal(1)
	m.voxels[Vector3i(0, 1, 0)] = GameMaterials.THATCH       # nor does anything that burns
	assert_int(w.open_faces(pit)).is_equal(1)
	m.voxels[Vector3i(0, 1, 0)] = GameMaterials.STONE
	assert_int(w.open_faces(pit)).is_equal(0)
	w.free()
	m.free()


func test_what_seals_a_pit() -> void:
	for id in [GameMaterials.DIRT, GameMaterials.GRASS, GameMaterials.SAND, GameMaterials.STONE, GameMaterials.CLAY, GameMaterials.DAUB_WALL, GameMaterials.GRAVEL_STATIC]:
		assert_bool(WorkSystem.seals(id)).override_failure_message(GameMaterials.name_of(id)).is_true()
	for id in [GameMaterials.AIR, GameMaterials.WATER, GameMaterials.WOOD, GameMaterials.THATCH, GameMaterials.WATTLE, GameMaterials.TALL_GRASS,
			GameMaterials.PEBBLES, GameMaterials.WORK_SPOT, GameMaterials.TORCH, GameMaterials.LEAVES]:
		assert_bool(WorkSystem.seals(id)).override_failure_message(GameMaterials.name_of(id)).is_false()


# ---------- pan ----------

func test_pan_and_sieve_have_their_own_loot() -> void:
	var pan := recipes.sift_table("gravel", "pan")
	var sieve := recipes.sift_table("gravel", "sieve")
	assert_array(pan).is_not_empty()
	assert_array(sieve).is_not_empty()
	for r in pan:
		assert_str(r["station"]).is_equal("pan")
	for r in sieve:
		assert_str(r.get("station", "sieve")).is_equal("sieve")
	assert_array(recipes.sift_table("sand", "pan")).is_not_empty()
	assert_str(WorkSystem._bed_item(GameMaterials.SAND_STATIC)).is_equal("sand")
	assert_bool(WorkSystem._is_water(GameMaterials.WATER_STATIC)).is_true()


# ---------- bag ----------

func test_bag_holds_overflow_and_comes_off_only_empty() -> void:
	var p := PlayerInventory.new(items, recipes)
	p.pickup("cobblestone", items.max_stack("cobblestone") * PlayerInventory.SIZE)
	assert_int(p.pickup("dirt", 5)).is_equal(5)   # full
	p.cursor = {"id": "leather_bag", "count": 1}
	p.click_bag_item()
	assert_bool(p.has_bag()).is_true()
	assert_dict(p.cursor).is_empty()
	assert_int(p.pickup("dirt", 5)).is_equal(0)
	assert_int(p.bag.count_of("dirt")).is_equal(5)
	p.click_bag_item()
	assert_bool(p.has_bag()).override_failure_message("a full bag stays on").is_true()
	p.bag.remove("dirt", 5)
	p.click_bag_item()
	assert_bool(p.has_bag()).is_false()
	assert_dict(p.cursor).is_equal({"id": "leather_bag", "count": 1})
	assert_str(p.summary()["bag"]).is_equal("")


func test_only_bags_go_in_the_bag_slot() -> void:
	var p := PlayerInventory.new(items, recipes)
	p.cursor = {"id": "dirt", "count": 3}
	p.click_bag_item()
	assert_bool(p.has_bag()).is_false()
	assert_dict(p.cursor).is_equal({"id": "dirt", "count": 3})


func test_quests_count_the_bag() -> void:
	var book := QuestBook.load_default(items)
	var p := PlayerInventory.new(items, recipes)
	p.bag_item = {"id": "leather_bag", "count": 1}
	p.bag.add("flint", 3)
	book.check_inventory(p.inv, p.cursor, [p.bag])
	assert_bool(book.is_done("gather_flint")).is_true()


# ---------- blocks: straw bed, storage pot, torch, shelter ----------

func test_straw_bed_is_furniture_you_sleep_in() -> void:
	assert_bool(FurnitureEntity.KINDS["straw_bed"]["sleep"]).is_true()
	var e := store.create_furniture(Vector3i(0, 0, 0), "straw_bed", 0)
	assert_int(e.cells.size()).is_equal(8)
	assert_array(e.contents()).is_equal([{"id": "straw_bed", "count": 1}])
	assert_float(e.seat().y).is_equal(1.0)
	var c := FurnitureEntity.cells_for("straw_bed")
	assert_bool(c.count(0) < c.size()).is_true()


func test_storage_pot_is_a_nine_slot_container_that_saves() -> void:
	var e: ChestEntity = store.create(Vector3i(2, 0, 0), "storage_pot")
	assert_int(e.inv.size()).is_equal(9)
	assert_str(e.title).is_equal("Storage Pot")
	e.inv.set_slot(8, {"id": "flint", "count": 5})
	var b2 := BlockEntities.new(items, recipes)
	b2.load_dict(JSON.parse_string(JSON.stringify(store.to_dict())))
	var e2: ChestEntity = b2.at(Vector3i(2, 0, 0))
	assert_str(e2.kind).is_equal("storage_pot")
	assert_dict(e2.inv.get_slot(8)).is_equal({"id": "flint", "count": 5})


func test_new_materials() -> void:
	var names := {GameMaterials.THATCH: "thatch", GameMaterials.WATTLE: "wattle", GameMaterials.DAUB_WALL: "daub_wall",
		GameMaterials.TORCH: "torch", GameMaterials.STORAGE_POT: "storage_pot"}
	for id in names:
		assert_str(GameMaterials.name_of(id)).is_equal(names[id])
	assert_int(GameMaterials.defs().size()).is_equal(GameMaterials.STORAGE_POT)
	var torch: Dictionary = GameMaterials.defs()[GameMaterials.TORCH - 1]
	assert_bool(torch.get("floor_only", false)).is_true()
	assert_str(torch["entity"]).is_equal("torch")
	assert_int(int(torch["tiles"][0])).is_equal(int(GameMaterials.defs()[GameMaterials.FIRE - 1]["tiles"][0]))   # the flame glows like fire
	for kind in ["wattle", "torch", "storage_pot"]:
		var cells := GameMaterials.micro_pattern(kind, GameMaterials.defs()[names.find_key(kind) - 1]["tiles"])
		assert_int(cells.size() - cells.count(0)).override_failure_message(kind).is_greater(20)
	for t in [88, 89, 90, 91, 92]:
		assert_int(t).is_less(GameMaterials.ATLAS_TILES.x * GameMaterials.ATLAS_TILES.y)
	var torch_e: TorchEntity = store.create(Vector3i(5, 0, 0), "torch")
	assert_array(store.all_of("torch")).is_equal([torch_e])
	assert_array(torch_e.contents()).is_empty()   # the torch item drops through mining.json
	assert_str(mining.drops("torch").get("id", "")).is_equal("torch")


func test_shelter_recipes() -> void:
	assert_str(recipes.match_grid(_grid(["dry_grass", "dry_grass", "", "dry_grass", "dry_grass"])).get("id", "")).is_equal("thatch")
	assert_str(recipes.match_grid(_grid(["stick", "stick", "", "stick", "stick"])).get("id", "")).is_equal("wattle")
	assert_str(recipes.match_grid(_grid(["clay_ball", "dry_grass", "dirt"])).get("id", "")).is_equal("daub")
	assert_str(recipes.match_grid(_grid(["wattle", "daub", "daub"])).get("id", "")).is_equal("daub_wall")
	assert_str(recipes.match_grid(_grid(["dry_grass", "dry_grass", "dry_grass", "dry_grass", "dry_grass", "dry_grass"]), "crafting_table").get("id", "")).is_equal("straw_bed")
	assert_str(recipes.match_grid(_grid(["stick", "dry_grass"])).get("id", "")).is_equal("torch_grass")
	assert_str(recipes.match_grid(_grid(["charcoal", "stick"])).get("id", "")).is_equal("torch_coal")
