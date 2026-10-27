extends GdUnitTestSuite

## Tools and ingots set down in the world (ADR 0095): which kind a sneaking right click makes, where a
## leaning tool stands, the ingot pile growing and giving back one at a time, per-stack data kept,
## and the pieces surviving a save.

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	store = BlockEntities.new(items, recipes)


func _pile(n: int) -> LaidEntity:
	var e: LaidEntity = store.create(Vector3i(0, 0, 0), "ingot_pile")
	e.stack = {"id": "copper_ingot", "count": n}
	return e


func test_kind_follows_item_and_face() -> void:
	var axe := items.get_def("copper_axe")
	var ingot := items.get_def("copper_ingot")
	assert_str(LaidEntity.kind_for(axe, Vector3i.UP)).is_equal("laid_tool")
	assert_str(LaidEntity.kind_for(axe, Vector3i(1, 0, 0))).is_equal("leaning_tool")
	assert_str(LaidEntity.kind_for(axe, Vector3i.DOWN)).is_equal("")
	assert_str(LaidEntity.kind_for(ingot, Vector3i.UP)).is_equal("ingot_pile")
	assert_str(LaidEntity.kind_for(ingot, Vector3i(0, 0, -1))).is_equal("")
	assert_str(LaidEntity.kind_for(items.get_def("firewood"), Vector3i.UP)).is_equal("")


## Flush with an off-grid wall on either side, on the block grid along it.
func test_lean_anchor_is_flush_with_the_wall() -> void:
	var b := Settings.BLOCK
	assert_object(LaidEntity.lean_anchor(Vector3i(5, 3, 6), Vector3i(1, 0, 0))).is_equal(Vector3i(5, 3, 4))
	assert_object(LaidEntity.lean_anchor(Vector3i(5, 3, 6), Vector3i(-1, 0, 0))).is_equal(Vector3i(5 - b + 1, 3, 4))
	assert_object(LaidEntity.lean_anchor(Vector3i(5, 3, 6), Vector3i(0, 0, -1))).is_equal(Vector3i(4, 3, 6 - b + 1))
	assert_int(LaidEntity.yaw_of(Vector3i(0, 0, 1))).is_equal(0)
	assert_int(LaidEntity.yaw_of(Vector3i(1, 0, 0))).is_equal(90)


func test_pile_grows_to_its_max_and_gives_back_one_at_a_time() -> void:
	var e := _pile(1)
	var ingot := {"id": "copper_ingot", "count": 5}
	assert_int(e.use(ingot)["add"]).is_equal(1)
	assert_int(int(e.stack["count"])).is_equal(2)
	assert_int(e.use({"id": "tin_ingot", "count": 5})["add"]).is_equal(0)   # one metal to a pile
	e.stack["count"] = Settings.ingot_pile_max
	assert_bool(e.is_full()).is_true()
	assert_int(e.use(ingot)["add"]).is_equal(0)
	var res := e.use({})
	assert_array(res["give"]).is_equal([{"id": "copper_ingot", "count": 1}])
	assert_bool(res["remove"]).is_false()
	e.stack["count"] = 1
	assert_bool(e.use({})["remove"]).is_true()


## A hot ingot does not join a cold pile (Inventory.same_kind, ADR 0087).
func test_hot_ingot_does_not_join_a_cold_pile() -> void:
	var e := _pile(3)
	var hot := Inventory.make_stack(items, "copper_ingot", 1, {"hot_until": 1.0e9})
	if Inventory.heat_of(hot) > 0.0:
		assert_int(e.use(hot)["add"]).is_equal(0)


func test_pile_cells_count_one_ingot_each() -> void:
	for n in [1, 8, 9, 64]:
		var filled := 0
		for v in LaidEntity.pile_cells(n, WorkPalette.COPPER):
			if v != 0:
				filled += 1
		assert_int(filled).is_equal(n * (4 * 8 + 2 * 6))   # a cast ingot: a full bottom, an inset top


## A tool comes back whole with its wear, the pile with its count, through BlockEntities' save.
func test_save_round_trip_keeps_stack_data() -> void:
	var tool: LaidEntity = store.create(Vector3i(8, 0, 0), "leaning_tool")
	tool.stack = Inventory.make_stack(items, "copper_axe", 1, {"wear": 7})
	tool.yaw = 90
	_pile(12)
	var d := store.to_dict()
	var back := BlockEntities.new(items, recipes)
	assert_bool(back.load_dict(JSON.parse_string(JSON.stringify(d)))).is_true()
	var t: LaidEntity = back.at(Vector3i(8, 3, 2))   # any voxel of its footprint finds it
	assert_str(t.kind).is_equal("leaning_tool")
	assert_int(int(t.stack["wear"])).is_equal(7)
	assert_int(t.yaw).is_equal(90)
	assert_int(int(back.at(Vector3i(0, 0, 0)).stack["count"])).is_equal(12)
	var give: Dictionary = t.use({})["give"][0]
	assert_str(give["id"]).is_equal("copper_axe")
	assert_int(int(give["wear"])).is_equal(7)   # JSON brings numbers back as floats, like every save


## Through WorkSystem's own click code: an empty hand takes an ingot, a click holding one adds it.
func test_work_system_takes_and_adds_ingots() -> void:
	var stub := GDScript.new()
	stub.source_code = "extends Node
var player_inv
var blocks
var player := {\"game_mode\": \"survival\"}
func get_world_voxel(_p): return GameMaterials.WORK_BLOCK
func set_world_voxel(_p, _m): pass
func spawn_item(_s, _a, _v): pass
"
	stub.reload()
	var main := Node.new()
	main.set_script(stub)
	main.blocks = store
	main.player_inv = PlayerInventory.new(items, recipes)
	var ws := WorkSystem.new()
	ws.main = main
	var e := _pile(2)
	ws._use_laid(e, null, false)
	assert_int(int(e.stack["count"])).is_equal(1)
	assert_str(main.player_inv.held()["id"]).is_equal("copper_ingot")
	ws._use_laid(e, null, true)
	assert_int(int(e.stack["count"])).is_equal(2)
	assert_bool(main.player_inv.held().is_empty()).is_true()
	e.stack["count"] = 1
	ws._use_laid(e, null, false)
	assert_object(store.at(Vector3i(0, 0, 0))).is_null()   # the last ingot taken: the pile is gone
	ws.free()
	main.free()
