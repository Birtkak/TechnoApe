extends GdUnitTestSuite

## ADR 0163: fishing data, the bite wait, species by water and biome, and the land traps' state and save
## (no world: the cast, reel and catches in the world are `/fish` and stone_ages_test).

var items: ItemRegistry
var recipes: RecipeRegistry
var fishing: Fishing


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	fishing = auto_free(Fishing.new())
	var data: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(Fishing.DATA))
	fishing.species = data["species"]
	fishing.junk = data["junk"]


func test_data_loads_clean() -> void:
	assert_array(items.errors).is_empty()
	assert_array(recipes.errors).is_empty()
	for id in ["snare", "pit_cover", "brown_trout", "pike", "cod"]:
		assert_bool(items.has(id)).is_true()
	for s in fishing.species:
		assert_bool(items.has(String(s["item"]))).is_true()
		assert_float(float(s["pull"])).is_greater(0.0)
	for j in fishing.junk:
		assert_bool(String(j["id"]) == "" or items.has(String(j["id"]))).is_true()
	# every whole fish fillets into raw fish, which the racks, salt and fires already take
	for id in items.all_ids():
		if items.has_tag(id, "whole_fish"):
			assert_bool(recipes.recipes.has("fillet_" + id)).is_true()


func test_more_fish_bite_sooner() -> void:
	assert_float(Fishing.bite_wait_for(0, 1.0)).is_equal(Settings.fish_bite_max_s)
	assert_float(Fishing.bite_wait_for(3, 1.0)).is_less(Fishing.bite_wait_for(1, 1.0))
	assert_float(Fishing.bite_wait_for(100, 1.0)).is_equal(Settings.fish_bite_min_s)


func test_species_by_water_and_biome() -> void:
	var sea: Array = fishing.candidates("sea", "ocean").map(func(s): return s["item"])
	assert_array(sea).contains(["cod", "mackerel"])
	assert_array(sea).not_contains(["pike", "carp"])
	var river: Array = fishing.candidates("river", "taiga").map(func(s): return s["item"])
	assert_array(river).contains(["raw_fish", "brown_trout"])
	assert_array(fishing.candidates("lake", "desert").map(func(s): return s["item"])).not_contains(["carp"])
	assert_array(fishing.candidates("lake", "swamp").map(func(s): return s["item"])).contains(["carp", "pike"])
	var list := [{"item": "a", "weight": 1}, {"item": "b", "weight": 3}]
	assert_str(Fishing.pick(list, 0.1)["item"]).is_equal("a")
	assert_str(Fishing.pick(list, 0.9)["item"]).is_equal("b")


func test_traps_spring_reset_and_save() -> void:
	var store := BlockEntities.new(items, recipes)
	var sn: TrapEntity = store.create(Vector3i(8, 100, 8), "snare")
	var pit: TrapEntity = store.create(Vector3i(16, 98, 16), "pit_trap")
	assert_bool(BlockEntities.is_work(sn)).is_true()
	assert_int(BlockEntities.footprint(sn).size()).is_equal(Settings.BLOCK * Settings.BLOCK)   # a voxel high
	assert_int(BlockEntities.footprint(pit).size()).is_equal(8 * 8 * 2)   # 1 m square, two voxels
	assert_vector(pit.centre()).is_equal(Vector3(20, 98, 20))
	assert_bool(sn.armed()).is_true()
	sn.spring("rabbit")
	pit.spring("")
	assert_bool(sn.armed()).is_false()
	assert_str(sn.look_lines()[0]).contains("Rabbit")
	assert_array(pit.contents()).is_empty()   # a fallen-in cover is gone
	var back := BlockEntities.new(items, recipes)
	assert_bool(back.load_dict(JSON.parse_string(JSON.stringify(store.to_dict())))).is_true()
	var sn2: TrapEntity = back.at(Vector3i(8, 100, 8))
	assert_str(sn2.catch).is_equal("rabbit")
	assert_bool((back.at(Vector3i(16, 98, 16)) as TrapEntity).sprung).is_true()
	sn2.reset()
	assert_bool(sn2.armed()).is_true()
	assert_array(sn2.contents()).is_equal([{"id": "snare", "count": 1}])
	assert_int((sn2.view_parts()["solid"]["cells"] as PackedInt32Array).size()).is_equal(512)


func test_trapped_mob_saves_its_hold() -> void:
	var m: Mob = auto_free(Mob.new())
	m.trap_at = Vector3(1.5, 2.0, 3.5)
	var d := m.to_dict()
	var m2: Mob = auto_free(Mob.new())
	m2.load_from(JSON.parse_string(JSON.stringify(d)))
	assert_vector(m2.trap_at).is_equal(Vector3(1.5, 2.0, 3.5))
	m.trap_at = Vector3.INF
	m2.load_from(JSON.parse_string(JSON.stringify(m.to_dict())))
	assert_bool(m2.trap_at == Vector3.INF).is_true()
