extends GdUnitTestSuite

## Headless tests for the Vintage Story early game (ADR 0023): recipe data,
## knapping, clay forming, pit kiln, firepit melting, casting, sieve, rock
## that drops nothing before copper, saving the pieces, and the whole chain
## from ground pickups to a copper pickaxe.

var items: ItemRegistry
var recipes: RecipeRegistry
var mining: MiningTable
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	mining = MiningTable.load_default()
	store = BlockEntities.new(items, recipes)


# ---------- data ----------

func test_data_loads_and_every_item_has_a_look() -> void:
	assert_array(items.errors).is_empty()
	assert_array(recipes.errors).is_empty()
	assert_array(mining.errors).is_empty()
	for t in ["knap", "clayform", "kiln", "melt", "cast", "sift"]:
		assert_array(recipes.of_type(t)).override_failure_message("no %s recipes" % t).is_not_empty()
	for id in ["wooden_pickaxe", "stone_pickaxe", "wooden_hoe", "stone_hoe"]:
		assert_bool(items.has(id)).override_failure_message("%s should be gone (ADR 0019)" % id).is_false()


func test_work_materials() -> void:
	assert_str(GameMaterials.name_of(GameMaterials.WORK_SPOT)).is_equal("work_spot")
	assert_str(GameMaterials.name_of(GameMaterials.WORK_BLOCK)).is_equal("work_block")
	assert_bool(MobManager.is_solid(GameMaterials.WORK_SPOT)).is_false()   # walked through
	assert_bool(MobManager.is_solid(GameMaterials.WORK_BLOCK)).is_true()
	assert_bool(mining.is_minable("work_spot")).is_true()


func test_every_mold_has_a_cast_and_every_raw_ware_a_kiln() -> void:
	for id in items.items_with_tag("raw_clay"):
		assert_dict(recipes.first_of("kiln", {"input": id})).override_failure_message("no kiln for " + id).is_not_empty()
		assert_dict(recipes.first_of("clayform", {"output": id})).override_failure_message("no clay form for " + id).is_not_empty()
	for id in items.items_with_tag("mold"):
		assert_dict(recipes.first_of("cast", {"mold": id})).override_failure_message("nothing casts in " + id).is_not_empty()


## Mold cavities must be closed on every side or the metal would run out.
func test_mold_cavities_are_enclosed() -> void:
	for r in recipes.of_type("clayform"):
		if not String(r["output"]["id"]).ends_with("_mold"):
			continue
		var top: Array = r["layers"][r["layers"].size() - 1]
		for x in 8:
			assert_str(String(top[0])[x]).is_equal("#")
			assert_str(String(top[7])[x]).is_equal("#")
		for z in 8:
			assert_str(String(top[z])[0]).is_equal("#")
			assert_str(String(top[z])[7]).is_equal("#")


# ---------- the path from nothing ----------

## Ground pickups, clay, cut grass and logs (with a flint axe) are all a new
## player can gather; from them every stone and copper age item must follow.
func test_copper_pickaxe_from_ground_pickups() -> void:
	var have := recipes.reachable_from(["flint", "stick", "loose_stone", "loose_copper_ore", "clay_ball", "dry_grass", "oak_log", "gravel", "sand", "plant_fiber"])
	for id in ["flint_knife", "flint_axe", "flint_pickaxe", "crafting_table", "firestarter", "firepit", "crucible",
			"pickaxe_mold", "crucible_copper", "copper_pickaxe_head", "copper_pickaxe", "copper_axe", "sieve", "copper_nugget"]:
		assert_bool(have.has(id)).override_failure_message("%s not reachable" % id).is_true()


func test_rock_drops_nothing_before_copper() -> void:
	var flint := items.get_def("flint_pickaxe")
	assert_float(mining.break_time("stone", flint)).is_equal_approx(2.25, 0.01)   # breaks at normal speed
	assert_array(mining.roll_drops("stone", flint, RandomNumberGenerator.new())).is_empty()
	assert_array(mining.roll_drops("stone", items.get_def("copper_pickaxe"), RandomNumberGenerator.new())).is_equal([{"id": "cobblestone", "count": 1}])
	# logs need an axe; the flint axe will do
	assert_dict(mining.drops("wood", {})).is_empty()
	assert_dict(mining.drops("wood", items.get_def("flint_axe"))).is_equal({"id": "oak_log", "count": 1})


func test_knife_cuts_dry_grass() -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = 1
	var by_hand := mining.roll_drops("tall_grass", {}, rng).filter(func(s: Dictionary) -> bool: return s["id"] == "dry_grass")
	assert_array(by_hand).is_empty()
	var cut := mining.roll_drops("tall_grass", items.get_def("flint_knife"), rng).filter(func(s: Dictionary) -> bool: return s["id"] == "dry_grass")
	assert_array(cut).is_equal([{"id": "dry_grass", "count": 1}])


# ---------- knapping ----------

func test_knapping_chips_only_marked_cells_and_finishes() -> void:
	var k := store.create(Vector3i(0, 10, 0), "knap") as KnapEntity
	assert_str(k.chip(0, 0)).is_equal("")   # no recipe yet
	k.set_recipe(recipes.recipes["knap_flint_axe_head"])
	var marked := k.remaining()
	assert_int(marked).is_greater(10)
	# a kept cell cannot be chipped
	assert_str(k.chip(2, 2)).is_equal("")
	var last := ""
	for z in 8:
		for x in 8:
			if k.is_marked(x, z):
				last = k.chip(x, z)
	assert_str(last).is_equal("done")
	assert_int(k.chips).is_equal(marked)
	assert_dict(k.output()).is_equal({"id": "flint_axe_head", "count": 1})
	assert_array(k.contents()).is_empty()   # chipped flint is spoilt


func test_unworked_knapping_gives_the_flint_back() -> void:
	var k := store.create(Vector3i(0, 10, 0), "knap") as KnapEntity
	assert_array(store.remove(k.pos)).is_equal([{"id": "flint", "count": 1}])


func test_nearest_marked_forgives_a_cell_of_aim() -> void:
	var k := store.create(Vector3i(0, 10, 0), "knap") as KnapEntity
	k.set_recipe(recipes.recipes["knap_flint_knife_blade"])
	# (3, 0) is kept in the knife pattern; aiming there chips its marked neighbour (2, 0)
	assert_bool(k.is_marked(3, 0)).is_false()
	assert_bool(k.is_marked(2, 0)).is_true()
	assert_vector(k.nearest_marked(3, 0, 1)).is_equal(Vector2i(2, 0))


# ---------- clay forming ----------

func test_clay_forming_fills_layers_and_charges_balls() -> void:
	var c := store.create(Vector3i(0, 10, 0), "clayform") as ClayFormEntity
	var r: Dictionary = recipes.recipes["form_crucible"]
	c.set_recipe(r)
	assert_int(c.active_layer()).is_equal(0)
	var balls := 0
	for i in 400:
		if c.is_done():
			break
		var y := c.active_layer()
		var placed := false
		for z in range(0, 8, 2):
			for x in range(0, 8, 2):
				var res := c.add(x, z, 99)
				balls += res["balls_used"]
				if res["added"] > 0:
					placed = true
		assert_bool(placed).override_failure_message("layer %d stuck" % y).is_true()
	assert_bool(c.is_done()).is_true()
	assert_int(balls).is_equal(ClayFormEntity.balls_for(r))
	assert_dict(c.output()).is_equal({"id": "raw_crucible", "count": 1})


func test_clay_forming_stops_without_clay() -> void:
	var c := store.create(Vector3i(0, 10, 0), "clayform") as ClayFormEntity
	c.set_recipe(recipes.recipes["form_ingot_mold"])
	var res := c.add(0, 0, 0)
	assert_int(res["added"]).is_equal(0)
	res = c.add(0, 0, 1)
	assert_int(res["added"]).is_equal(4)
	assert_int(c.credit).is_equal(ClayFormEntity.CELLS_PER_BALL - 4)


# ---------- pit kiln and casting ----------

func _ground(item: String) -> GroundEntity:
	var g := store.create(Vector3i(0, 10, 0), "ground") as GroundEntity
	g.item = item
	return g


func test_pit_kiln_needs_grass_sticks_and_fire() -> void:
	var g := _ground("raw_crucible")
	assert_str(g.use({"id": "stick", "tags": ["stick"]})["msg"]).contains("dry grass")
	for i in GroundEntity.KILN_GRASS:
		assert_int(g.use(items.get_def("dry_grass"))["consume"]).is_equal(1)
	for i in GroundEntity.KILN_STICKS:
		assert_int(g.use(items.get_def("stick"))["consume"]).is_equal(1)
	assert_bool(g.is_lit()).is_false()
	g.use(items.get_def("firestarter"))
	assert_bool(g.is_lit()).is_true()
	for i in int(Settings.pit_kiln_burn_s * 20.0):
		store.tick()
	assert_bool(g.is_lit()).is_false()
	assert_str(g.item).is_equal("crucible")
	assert_array(g.contents()).is_equal([{"id": "crucible", "count": 1}])   # the fuel burnt


func test_casting_returns_the_crucible_and_cools() -> void:
	var g := _ground("pickaxe_mold")
	var res := g.use(items.get_def("crucible_copper"))
	assert_str(res["replace_held"]).is_equal("crucible")
	assert_str(g.cast_output).is_equal("copper_pickaxe_head")
	assert_str(g.use({})["msg"]).contains("hot")
	for i in int(Settings.cast_cool_s * 20.0):
		store.tick()
	res = g.use({})
	assert_array(res["give"]).is_equal([{"id": "copper_pickaxe_head", "count": 1}])
	assert_str(g.item).is_equal("pickaxe_mold")   # the mold stays
	res = g.use({})
	assert_bool(res["remove"]).is_true()          # an empty hand picks the mold up


func test_wrong_mold_does_not_take_metal() -> void:
	var g := _ground("raw_pickaxe_mold")   # not fired yet
	var res := g.use(items.get_def("crucible_copper"))
	assert_str(res["replace_held"]).is_equal("")
	assert_str(g.cast_output).is_equal("")


# ---------- firepit ----------

func test_firepit_needs_a_firestarter_then_melts_copper() -> void:
	var f := store.create(Vector3i(0, 10, 0), "firepit") as FirepitEntity
	f.inv.set_slot(FirepitEntity.FUEL, {"id": "oak_log", "count": 8 * items.units("oak_log")})   # eight logs of wood voxels (ADR 0034)
	f.inv.set_slot(FirepitEntity.VESSEL, {"id": "crucible", "count": 1})
	f.inv.set_slot(FirepitEntity.INPUT, {"id": "loose_copper_ore", "count": 5})
	for i in 100:
		store.tick()
	assert_bool(f.is_lit()).is_false()   # no fire by itself
	assert_bool(f.ignite()).is_true()
	for i in 401:
		store.tick()
	assert_dict(f.inv.get_slot(FirepitEntity.VESSEL)).is_equal({"id": "crucible_copper", "count": 1})
	assert_dict(f.inv.get_slot(FirepitEntity.INPUT)).is_equal({"id": "loose_copper_ore", "count": 1})


func test_firepit_cooks_and_burns_out() -> void:
	var f := store.create(Vector3i(0, 10, 0), "firepit") as FirepitEntity
	assert_bool(f.ignite()).is_false()   # no fuel
	f.inv.set_slot(FirepitEntity.FUEL, {"id": "stick", "count": 1})
	f.inv.set_slot(FirepitEntity.INPUT, {"id": "raw_meat", "count": 1})
	assert_bool(f.ignite()).is_true()
	for i in 99:
		store.tick()
	assert_bool(f.is_lit()).is_true()
	store.tick()
	assert_bool(f.is_lit()).is_false()   # one stick = 100 ticks, not enough to cook (300)
	f.inv.set_slot(FirepitEntity.FUEL, {"id": "oak_log", "count": 2})
	f.ignite()
	for i in 301:
		store.tick()
	assert_dict(f.inv.get_slot(FirepitEntity.OUTPUT)).is_equal({"id": "cooked_meat", "count": 1})


func test_firepit_slots() -> void:
	var f := store.create(Vector3i(0, 10, 0), "firepit") as FirepitEntity
	assert_bool(f.may_place(FirepitEntity.VESSEL, "crucible")).is_true()
	assert_bool(f.may_place(FirepitEntity.VESSEL, "loose_copper_ore")).is_false()
	assert_array(f.shift_targets("crucible")).is_equal([FirepitEntity.VESSEL])
	assert_array(f.shift_targets("loose_copper_ore")).is_equal([FirepitEntity.INPUT])
	assert_array(f.shift_targets("oak_log")).is_equal([FirepitEntity.FUEL])


# ---------- sieve ----------

func test_sieve_loads_shakes_and_rolls_the_table() -> void:
	var s := store.create(Vector3i(0, 10, 0), "sieve") as SieveEntity
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	assert_str(s.use(items.get_def("dirt"), rng)["msg"]).contains("gravel")
	var got := {}
	for n in 200:
		assert_int(s.use(items.get_def("gravel"), rng)["consume"]).is_equal(1)
		for i in SieveEntity.SHAKES - 1:
			assert_array(s.use({}, rng)["drops"]).is_empty()
		for d in s.use({}, rng)["drops"]:
			got[d["id"]] = got.get(d["id"], 0) + 1
		assert_str(s.input).is_equal("")
	# 200 sifts: flint ~50, copper ~36, iron ~8
	assert_int(got.get("flint", 0)).is_between(30, 75)
	assert_int(got.get("copper_nugget", 0)).is_between(18, 56)
	assert_int(got.get("iron_nugget", 0)).is_between(1, 20)
	assert_array(recipes.sift_table("sand")).is_not_empty()


# ---------- saving ----------

func test_pieces_survive_save_and_load() -> void:
	var n := Settings.BLOCK   # pieces are blocks: anchors one block apart (ADR 0034)
	var k := store.create(Vector3i(1 * n, 10, 0), "knap") as KnapEntity
	k.set_recipe(recipes.recipes["knap_flint_hoe_head"])
	k.chip(0, 0)
	var c := store.create(Vector3i(2 * n, 10, 0), "clayform") as ClayFormEntity
	c.set_recipe(recipes.recipes["form_axe_mold"])
	c.add(0, 0, 1)
	var g := _ground("raw_ingot_mold")
	g.use(items.get_def("dry_grass"))
	var f := store.create(Vector3i(3 * n, 10, 0), "firepit") as FirepitEntity
	f.inv.set_slot(FirepitEntity.VESSEL, {"id": "crucible", "count": 1})
	var s := store.create(Vector3i(4 * n, 10, 0), "sieve") as SieveEntity
	s.input = "gravel"
	s.shakes = 3
	var d := store.to_dict()
	var other := BlockEntities.new(items, recipes)
	assert_bool(other.load_dict(JSON.parse_string(JSON.stringify(d)))).is_true()
	var k2 := other.at(Vector3i(1 * n, 10, 0)) as KnapEntity
	assert_str(k2.recipe.get("id", "")).is_equal("knap_flint_hoe_head")
	assert_int(k2.chips).is_equal(1)
	var c2 := other.at(Vector3i(2 * n, 10, 0)) as ClayFormEntity
	assert_int(c2.placed).is_equal(4)
	assert_bool(c2.is_filled(0, 0, 0)).is_true()
	assert_int((other.at(Vector3i(0, 10, 0)) as GroundEntity).grass).is_equal(1)
	assert_dict((other.at(Vector3i(3 * n, 10, 0)) as FirepitEntity).inv.get_slot(FirepitEntity.VESSEL)).is_equal({"id": "crucible", "count": 1})
	assert_int((other.at(Vector3i(4 * n, 10, 0)) as SieveEntity).shakes).is_equal(3)
	assert_int(other.all_work().size()).is_equal(5)


# ---------- views ----------

func test_every_piece_draws() -> void:
	var n := Settings.BLOCK
	var k := store.create(Vector3i(1 * n, 10, 0), "knap") as KnapEntity
	k.set_recipe(recipes.recipes["knap_flint_axe_head"])
	var c := store.create(Vector3i(2 * n, 10, 0), "clayform") as ClayFormEntity
	c.set_recipe(recipes.recipes["form_crucible"])
	var g := _ground("pickaxe_mold")
	g.use(items.get_def("crucible_copper"))
	store.create(Vector3i(3 * n, 10, 0), "firepit")
	store.create(Vector3i(4 * n, 10, 0), "sieve")
	for e in store.all_work():
		var parts: Dictionary = e.view_parts()
		assert_bool(parts.has("solid")).is_true()
		var v := WorkPalette.make_view(e.pos, parts["solid"], parts.get("ghost", {}), parts.get("guide", {}))
		assert_int(v.get_child_count()).override_failure_message("%s draws nothing" % e.kind).is_greater(0)
		v.free()
	# the fresh clay form shows its first layer as a guide only
	var bare := WorkPalette.make_view(c.pos, c.view_parts()["solid"], {})
	assert_int(bare.get_child_count()).is_equal(0)
	bare.free()


func test_plane_cell_maps_the_aim_to_cells() -> void:
	# looking straight down at the middle of the piece anchored at (4, 10, 7) (a block of
	# BLOCK voxels, ADR 0034), slab top one cell up
	var h := Settings.BLOCK * 0.5
	var c = WorkSystem.plane_cell(Vector3i(4, 10, 7), 1.0, Vector3(4 + h, 14.0, 7 + h), Vector3(0, -1, 0))
	assert_vector(c).is_equal_approx(Vector2(4, 4), Vector2(0.001, 0.001))
	assert_that(WorkSystem.plane_cell(Vector3i(4, 10, 7), 1.0, Vector3(4 + h, 14.0, 7 + h), Vector3(0, 1, 0))).is_null()
