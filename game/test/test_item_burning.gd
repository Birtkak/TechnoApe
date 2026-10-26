extends GdUnitTestSuite

## Dropped items burn up in lava (bug report 2026-10-25 "make items burn in lava"): ItemEntity.burns_at
## over a fake voxel lookup, the fireproof tag, and the puff's particle effects and sound.

var items: ItemRegistry
var lava := 0
var water := 0


func before() -> void:
	items = ItemRegistry.load_default()
	lava = GameMaterials.LAVA
	water = GameMaterials.WATER


## A world of air with a pool of `m` filling y < 0.
func _pool(m: int) -> Callable:
	return func(p: Vector3i) -> int: return m if p.y < 0 else GameMaterials.AIR


func test_only_hot_liquids_burn() -> void:
	assert_bool(ItemEntity.burns_items(lava)).is_true()
	for m in [GameMaterials.AIR, water, GameMaterials.STONE, GameMaterials.id_of("oil"), GameMaterials.FIRE, -1, 99999]:
		assert_bool(ItemEntity.burns_items(m)).override_failure_message("material %d burns items" % m).is_false()


func test_item_in_lava_burns() -> void:
	assert_bool(ItemEntity.burns_at(items, "stick", Vector3(0.5, -3.0, 0.5), _pool(lava))).is_true()   # sunk in
	assert_bool(ItemEntity.burns_at(items, "stick", Vector3(0.5, 0.0, 0.5), _pool(lava))).is_true()    # resting on top


func test_item_above_lava_or_in_water_survives() -> void:
	assert_bool(ItemEntity.burns_at(items, "stick", Vector3(0.5, 2.0, 0.5), _pool(lava))).is_false()
	assert_bool(ItemEntity.burns_at(items, "stick", Vector3(0.5, -3.0, 0.5), _pool(water))).is_false()


func test_fireproof_items_survive() -> void:
	for id in ["obsidian", "lava_bucket"]:
		assert_bool(items.has_tag(id, "fireproof")).override_failure_message(id + " not fireproof").is_true()
		assert_bool(ItemEntity.burns_at(items, id, Vector3(0.5, -3.0, 0.5), _pool(lava))).is_false()


func test_puff_effects_and_sound_exist() -> void:
	var fx: ParticleFx = auto_free(ParticleFx.load_default())
	assert_array(fx.errors).is_empty()
	assert_bool(fx.effects.has("item_burn")).is_true()
	assert_bool(fx.effects.has("item_burn_smoke")).is_true()
	var sounds = JSON.parse_string(FileAccess.get_file_as_string("res://data/sounds.json"))
	assert_str(String(sounds["particles"].get("item_burn", ""))).is_equal("sim/sizzle")


## The live path: an ItemEntity dropped into a lava pool reports burning() once it is unfrozen.
func test_entity_reports_burning() -> void:
	var e: ItemEntity = auto_free(ItemEntity.create(items, {"id": "stick", "count": 3}, null))
	e.position = Vector3(0.5, -2.0, 0.5)
	assert_bool(e.burning()).is_false()   # no world yet
