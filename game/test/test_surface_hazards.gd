extends GdUnitTestSuite

## Surface hazards (ADR 0149): the strike target choice and rod grounding (Weather.pick_target over sampled
## cells), the rod's material id and pole stamp, the item and recipe, and the commands' registration.


## A surface map of `cols` x `cols` columns from (0, 0) one voxel apart, flat at y 40 with the given tops.
static func _map(cols: int, tops: Dictionary) -> PackedInt32Array:
	var m := PackedInt32Array()
	m.resize(cols * cols)
	for i in m.size():
		var y: int = tops.get(Vector2i(i % cols, i / cols), 40)
		m[i] = GameMaterials.STONE | (y << 16) if y >= 0 else 0
	return m


func test_strike_picks_the_highest_surface_voxel() -> void:
	var m := _map(8, {Vector2i(3, 3): 70, Vector2i(6, 1): 55, Vector2i(2, 7): -1})   # a tree top, a roof, a column not yet loaded
	var t := Weather.pick_target(m, Vector2i(100, 200), 8, 1, [], 10.0)
	assert_that(t["pos"]).is_equal(Vector3i(103, 70, 203))
	assert_bool(t["rod"]).is_false()
	var stepped := Weather.pick_target(m, Vector2i(0, 0), 8, 4, [], 10.0)   # columns 4 voxels apart
	assert_that(stepped["pos"]).is_equal(Vector3i(12, 70, 12))


func test_nothing_loaded_gives_no_target() -> void:
	var m := PackedInt32Array([0, 0, 0, 0])
	var t := Weather.pick_target(m, Vector2i.ZERO, 2, 1, [Vector3i(1, 50, 1)], 10.0)
	assert_int(t["pos"].y).is_less(0)
	assert_bool(t["rod"]).is_false()


func test_rod_within_reach_grounds_the_strike_and_a_far_one_does_not() -> void:
	var m := _map(8, {Vector2i(3, 3): 70})
	var near := Weather.pick_target(m, Vector2i.ZERO, 8, 1, [Vector3i(9, 60, 3)], 10.0)   # 6 voxels off, lower: still wins
	assert_that(near["pos"]).is_equal(Vector3i(9, 60, 3))
	assert_bool(near["rod"]).is_true()
	var far := Weather.pick_target(m, Vector2i.ZERO, 8, 1, [Vector3i(30, 60, 3)], 10.0)
	assert_that(far["pos"]).is_equal(Vector3i(3, 70, 3))
	assert_bool(far["rod"]).is_false()
	var two := Weather.pick_target(m, Vector2i.ZERO, 8, 1, [Vector3i(11, 60, 3), Vector3i(6, 60, 3)], 10.0)
	assert_that(two["pos"]).is_equal(Vector3i(6, 60, 3))   # the nearest rod


func test_rod_material_id_item_and_pole() -> void:
	assert_int(GameMaterials.id_of("lightning_rod")).is_equal(139)   # reserved for ADR 0149; append only
	var rod := GameMaterials.id_of("lightning_rod")
	var st := PlantStamps.for_placed(rod, Vector3i(5, 60, 5))
	assert_int(st.size()).is_equal(PlantStamps.ROD_H - 1)
	assert_int(st[Vector3i(0, PlantStamps.ROD_H - 1, 0)]).is_equal(rod)
	var e := LightningRodEntity.new(Vector3i(5, 60, 5))
	assert_that(e.tip()).is_equal(Vector3i(5, 60 + PlantStamps.ROD_H - 1, 5))
	var items := ItemRegistry.load_default()
	assert_that(items.get_def("lightning_rod").get("material", "")).is_equal("lightning_rod")
	var recipes := RecipeRegistry.load_default(items)
	assert_bool(recipes.recipes.has("lightning_rod")).is_true()


func test_commands_registered() -> void:
	var reg := CommandRegistry.new()
	var inv := PlayerInventory.new(ItemRegistry.load_default(), RecipeRegistry.load_default(ItemRegistry.load_default()))
	var player: Player = auto_free(Player.new())
	player.inventory = inv
	var cmds := GameCommands.new(reg, player, inv, null)
	assert_bool(reg.find("lightning").is_empty()).is_false()
	assert_bool(reg.find("erupt").is_empty()).is_false()
	assert_bool(reg.execute("/erupt")["ok"]).is_false()   # no world: fails politely
	assert_bool(reg.execute("/lightning here")["ok"]).is_false()
	assert_object(cmds).is_not_null()
