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


## Stone below y = 0 with holes cut by `hole` (Vector3i -> bool): Fishing.pit_spot reads it through main.
class StubWorld:
	extends Node
	var hole: Callable

	func get_world_voxel(p: Vector3i) -> int:
		return GameMaterials.STONE if p.y < 0 and not hole.call(p) else GameMaterials.AIR


func _pit(hole: Callable, aim: Vector3i) -> Dictionary:
	var w: StubWorld = auto_free(StubWorld.new())
	w.hole = hole
	fishing.main = w
	return fishing.pit_spot(aim)


func test_pit_cover_fits_square_round_and_ragged_holes() -> void:
	var square := func(p: Vector3i) -> bool: return p.y >= -12 and p.x >= 0 and p.x < 8 and p.z >= 0 and p.z < 8
	var s := _pit(square, Vector3i(3, -11, 4))
	assert_vector(Vector3(s["anchor"])).is_equal(Vector3(0, -2, 0))   # flush with the rim
	assert_int(s["depth"]).is_equal(12)
	var round := func(p: Vector3i) -> bool: return p.y >= -10 and Vector2(p.x + 0.5, p.z + 0.5).length() < 5.6
	assert_dict(_pit(round, Vector3i(0, -9, 0))).is_not_empty()   # 1.4 m across
	var ragged := func(p: Vector3i) -> bool: return p.y >= -9 and p.x >= -1 - (p.z & 1) and p.x < 9 and p.z >= 0 and p.z < 9 + (p.x & 1)
	assert_dict(_pit(ragged, Vector3i(4, -8, 4))).is_not_empty()
	var wide := func(p: Vector3i) -> bool: return p.y >= -12 and absi(p.x) < 10 and absi(p.z) < 10   # 2.5 m: the cover falls in
	assert_dict(_pit(wide, Vector3i(0, -11, 0))).is_empty()
	var shallow := func(p: Vector3i) -> bool: return p.y >= -5 and p.x >= 0 and p.x < 8 and p.z >= 0 and p.z < 8
	assert_dict(_pit(shallow, Vector3i(3, -4, 3))).is_empty()
	var narrow := func(p: Vector3i) -> bool: return p.y >= -12 and p.x >= 0 and p.x < 6 and p.z >= 0 and p.z < 6
	assert_dict(_pit(narrow, Vector3i(3, -11, 3))).is_empty()   # the 1 m cover does not fit in
