extends GdUnitTestSuite

## Furniture and storage of ADR 0121: plank furniture in every wood, the bench and candle, the barrel.

var items: ItemRegistry
var recipes: RecipeRegistry


func before() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)


func _grid(id: String, n: int) -> Array:
	var g := []
	for i in 9:
		g.append(id if i < n else "")
	return g


func test_every_wood_has_its_furniture() -> void:
	for w in FurnitureEntity.WOODS:
		for k in FurnitureEntity.WOODED:
			var kind := "%s_%s" % [w, k]
			assert_bool(FurnitureEntity.KINDS.has(kind)).override_failure_message(kind).is_true()
			assert_str(String(items.get_def(kind).get("furniture", ""))).is_equal(kind)
			var oak := FurnitureEntity.cells_for(k)
			var mine := FurnitureEntity.cells_for(kind)
			assert_int(mine.size()).is_equal(oak.size())
			assert_bool(mine != oak).override_failure_message(kind + " looks like oak").is_true()
			for i in mine.size():   # the same shape: only the paints change
				assert_bool((mine[i] == 0) == (oak[i] == 0)).is_true()
				if mine[i] != oak[i]:
					break
	# six planks of a wood make that wood's door; oak planks the oak door
	assert_str(recipes.match_grid(_grid("spruce_planks", 6), "crafting_table").get("id", "")).is_equal("spruce_door")
	assert_str(recipes.match_grid(_grid("oak_planks", 6), "crafting_table").get("id", "")).is_equal("door")
	assert_int(FurnitureEntity.ATLAS.y).is_equal(FurnitureEntity.PALETTE.size())


func test_bench_seats_and_candle_lights() -> void:
	assert_bool(FurnitureEntity.sits("bench")).is_true()
	assert_bool(FurnitureEntity.sits("birch_bench")).is_true()
	assert_bool(FurnitureEntity.sits("chair")).is_true()
	assert_bool(FurnitureEntity.sits("table")).is_false()
	assert_bool(items.get_def("candle").has("light")).is_true()
	assert_int(FurnitureEntity.cells_for("candle").count(FurnitureEntity.FLAME + 1)).is_greater(0)


func test_barrel_is_storage() -> void:
	assert_int(GameMaterials.BARREL).is_equal(216)
	assert_str(GameMaterials.name_of(GameMaterials.BARREL)).is_equal("barrel")
	assert_bool(GameMaterials.is_station(GameMaterials.BARREL)).is_true()
	var e := ChestEntity.new(items, Vector3i.ZERO, "barrel")
	assert_int(e.inv.size()).is_equal(18)
	assert_int(BlockEntities.footprint_of("barrel", Vector3i.ZERO).size()).is_equal(6 * 8 * 6)
	var cs: Array = StationEntity.cells("barrel")
	assert_int((cs[0] as PackedInt32Array).size() - (cs[0] as PackedInt32Array).count(0)).is_greater(100)
	assert_str(String(MiningTable.load_default().entry("barrel").get("drops", ""))).is_equal("barrel")


func test_rugs_lie_flat() -> void:
	for k in ["red_rug", "blue_rug", "straw_mat"]:
		var c := FurnitureEntity.cells_for(k)
		assert_int(c.count(0)).override_failure_message(k + " has holes").is_equal(0)
		assert_int(FurnitureEntity.KINDS[k]["box"][1].y).is_equal(1)
	assert_str(recipes.match_grid(["wool", "wool", "cornflower", "", "", "", "", "", ""], "").get("id", "")).is_equal("blue_rug")
	assert_str(recipes.match_grid(["dry_grass", "dry_grass", "dry_grass", "", "", "", "", "", ""], "").get("id", "")).is_equal("straw_mat")


func test_signs_keep_their_words() -> void:
	assert_bool(FurnitureEntity.KINDS["birch_sign"].get("write", false)).is_true()
	var e := FurnitureEntity.new(Vector3i.ZERO, "sign")
	e.text = "Mine ->"
	var back := FurnitureEntity.new(Vector3i.ZERO)
	back.load_from(e.to_dict())
	assert_str(back.text).is_equal("Mine ->")
	assert_str(recipes.match_grid(["oak_planks", "oak_planks", "oak_planks", "oak_planks", "oak_planks", "oak_planks", "", "stick", ""],
		"crafting_table").get("id", "")).is_equal("sign")
